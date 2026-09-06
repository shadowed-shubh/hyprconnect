
use anyhow::{Context,Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tracing::info;

const SERVICE_TYPE: &str = "_hyprconnect._tcp.local.";

pub struct DiscoveryIdentity{
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub protocol_version: u32,
    pub port: u16,
}

pub async fn run(identity: DiscoveryIdentity) -> Result<()>{
    let mdns = ServiceDaemon::new().context("failed to start mDNS daemon")?;
    let host_name = format!("{}.local.",identity.device_id);
    let properties = [
        ("device_id", identity.device_id.as_str()),
        ("device_name", identity.device_name.as_str()),
        ("device_type", identity.device_type.as_str()),
        ("protocol_version", &identity.protocol_version.to_string()),
    ] ;

    let service_info = ServiceInfo::new(
        SERVICE_TYPE,
        &identity.device_id,
        &host_name,
        "",
        identity.port,
        &properties[..],
    )
    .context("failed to build mDNS service info")?
    .enable_addr_auto();

    mdns.register(service_info)
        .context("failed to register mDNS service")?;

    info!(
        "advertising as {} ({}) on port {}",
        identity.device_name, identity.device_id, identity.port
    );
    let receiver = mdns 
        .browse(SERVICE_TYPE)
        .context("failed to start mDNS browse")?;

    loop {
        match receiver.recv_async().await {

            Ok(ServiceEvent::ServiceResolved(info)) => {
                let props = info.get_properties();
                let remote_id = props
                    .get_property_val_str("device_id")
                    .unwrap_or("unknown");

                if remote_id == identity.device_id{
                    continue;
                }

                let remote_name = props
                    .get_property_val_str("device_name")
                    .unwrap_or("unknown");

                info!("discovered device: {} ({}) at {:?}",
                    remote_name,
                    remote_id,
                    info.get_addresses()
                    );
            }
            Ok(_) => {}
            Err(_) => {break;}
        }
    }
    Ok(())
}

