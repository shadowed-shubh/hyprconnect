use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::broadcast;
use tracing::{info, warn};
use zbus::{Connection, proxy};

use crate::packet::Packet;

#[proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower/devices/DisplayDevice"
)]
trait UPowerDevice {
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
}

// UPower state codes: 2 = Discharging, 6 = Pending discharge.
// Everything else (charging, full, pending charge, unknown) counts
// as "not actively draining" for our purposes.
fn state_is_charging(state: u32) -> bool {
    !matches!(state, 2 | 6)
}

pub async fn read_battery_state() -> Result<(i64, bool)> {
    let conn = Connection::system()
        .await
        .context("failed to connect to system DBus")?;
    let proxy = UPowerDeviceProxy::new(&conn)
        .await
        .context("failed to create UPower proxy")?;
    let percentage = proxy
        .percentage()
        .await
        .context("failed to read Percentage")?;
    let state = proxy.state().await.context("failed to read State")?;
    Ok((percentage.round() as i64, state_is_charging(state)))
}

pub async fn handle_request_body() -> serde_json::Value {
    match read_battery_state().await {
        Ok((level, charging)) => {
            info!(
                "reporting real battery via UPower: {}% charging={}",
                level, charging
            );
            json!({ "level": level, "charging": charging })
        }
        Err(e) => {
            warn!("could not read battery via UPower: {e:#}");
            json!({ "level": null, "charging": null, "error": "no_battery" })
        }
    }
}

pub fn request() -> Packet {
    Packet::new("BATTERY_REQUEST", json!({}))
}

pub fn response(body: serde_json::Value) -> Packet {
    Packet::new("BATTERY_RESPONSE", body)
}

pub fn is_request(packet: &Packet) -> bool {
    packet.packet_type == "BATTERY_REQUEST"
}

pub fn is_response(packet: &Packet) -> bool {
    packet.packet_type == "BATTERY_RESPONSE"
}

pub fn is_changed(packet: &Packet) -> bool {
    packet.packet_type == "BATTERY_CHANGED"
}

pub fn handle_response(packet: &Packet) -> Result<()> {
    let level = packet.body.get("level").and_then(|v| v.as_i64());
    let charging = packet.body.get("charging").and_then(|v| v.as_bool());
    match level {
        Some(l) => info!(
            "received {} — level: {}%, charging: {}",
            packet.packet_type,
            l,
            charging.unwrap_or(false)
        ),
        None => info!("peer reports no battery"),
    }
    Ok(())
}

// Runs forever, watching UPower for real percentage/state changes,
// and broadcasts a BATTERY_CHANGED packet to every active session
// whenever something actually changes — this is the real "push"
// behavior, not polling.
pub async fn spawn_watcher(tx: broadcast::Sender<Packet>) -> Result<()> {
    let conn = Connection::system()
        .await
        .context("failed to connect to system DBus")?;
    let proxy = UPowerDeviceProxy::new(&conn)
        .await
        .context("failed to create UPower proxy")?;

    let mut last_level = proxy.percentage().await.ok().map(|p| p.round() as i64);
    let mut last_charging = proxy.state().await.ok().map(state_is_charging);

    let mut pct_changes = proxy.receive_percentage_changed().await;
    let mut state_changes = proxy.receive_state_changed().await;

    loop {
        tokio::select! {
            Some(change) = pct_changes.next() => {
                if let Ok(pct) = change.get().await {
                    last_level = Some(pct.round() as i64);
                    broadcast_if_ready(&tx, last_level, last_charging).await;
                }
            }
            Some(change) = state_changes.next() => {
                if let Ok(state) = change.get().await {
                    last_charging = Some(state_is_charging(state));
                    broadcast_if_ready(&tx, last_level, last_charging).await;
                }
            }
            else => break,
        }
    }
    Ok(())
}

async fn broadcast_if_ready(
    tx: &broadcast::Sender<Packet>,
    level: Option<i64>,
    charging: Option<bool>,
) {
    if let (Some(level), Some(charging)) = (level, charging) {
        info!(
            "battery changed: {}% charging={} — broadcasting",
            level, charging
        );
        let packet = Packet::new(
            "BATTERY_CHANGED",
            json!({ "level": level, "charging": charging }),
        );
        let _ = tx.send(packet);
    }
}
