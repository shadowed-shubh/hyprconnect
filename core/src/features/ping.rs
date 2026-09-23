use crate::packet::Packet;
use serde_json::json;

pub const PING: &str = "PING";
pub const PONG: &str = "PONG";

pub fn ping() -> Packet {
    Packet::new(PING, json!({}))
}
pub fn pong() -> Packet {
    Packet::new(PONG, json!({}))
}
pub fn is_ping(packet: &Packet) -> bool {
    packet.packet_type == PING
}
pub fn is_pong(packet: &Packet) -> bool {
    packet.packet_type == PONG
}
