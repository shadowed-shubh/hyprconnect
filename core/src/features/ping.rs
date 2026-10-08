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

#[cfg(test)]
mod tests {
    use super::{is_ping, is_pong, ping, pong, PING, PONG};

    #[test]
    fn ping_and_pong_route_to_their_matching_handlers() {
        assert_eq!(ping().packet_type, PING);
        assert_eq!(pong().packet_type, PONG);
        assert!(is_ping(&ping()));
        assert!(!is_pong(&ping()));
        assert!(is_pong(&pong()));
        assert!(!is_ping(&pong()));
    }
}
