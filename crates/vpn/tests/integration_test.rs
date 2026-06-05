//! P0-4: 闆嗘垚娴嬭瘯
//!
//! 娴嬭瘯鍦板潃瑙ｆ瀽 + LAN 璺敱鐨勫畬鏁存祦绋嬶紝
//! 楠岃瘉 `ll ping`, `ll cmd`, `ll file` 鍔熻兘鐨勮矾鐢卞眰鏀寔銆?

use lan_link_vpn::address::{resolve_address, AddressResolver, MemAddressResolver};
use lan_link_vpn::lan_router::{LanRouter, UdpMessageType};
use lan_link_vpn::router::{ConnectionType, NodeStatus, Router, RouterError, RouterStatus};
use lan_link_vpn::vpn::identity::NodeID;
use lan_link_vpn::vpn::dht::DhtManager;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ===================== 鍦板潃瑙ｆ瀽闆嗘垚娴嬭瘯 =====================

#[test]
fn test_full_address_pipeline_simple() {
    let resolver = MemAddressResolver::new();
    let id = NodeID::from_bytes(&[0xAB; 32]);
    resolver.add_static_mapping("Pikachu", id);

    // 瀹屾暣娴佺▼锛氳В鏋愬湴鍧€瀛楃涓?-> 鑾峰彇鑺傜偣 ID
    let (parsed, resolved_id) = resolve_address(&resolver, "node:Pikachu").unwrap();
    assert_eq!(parsed.name, "Pikachu");
    assert_eq!(parsed.port, None);
    assert_eq!(resolved_id, id);
}

#[test]
fn test_full_address_pipeline_with_port() {
    let resolver = MemAddressResolver::new();
    let id = NodeID::from_bytes(&[0xCD; 32]);
    resolver.add_static_mapping("Charizard", id);

    let (parsed, resolved_id) = resolve_address(&resolver, "node:Charizard:8080").unwrap();
    assert_eq!(parsed.name, "Charizard");
    assert_eq!(parsed.port, Some(8080));
    assert_eq!(resolved_id, id);
}

#[test]
fn test_multiple_node_resolution() {
    let resolver = MemAddressResolver::new();

    let pikachu_id = NodeID::from_bytes(&[1; 32]);
    let charizard_id = NodeID::from_bytes(&[2; 32]);
    let mewtwo_id = NodeID::from_bytes(&[3; 32]);

    resolver.add_static_mapping("Pikachu", pikachu_id);
    resolver.add_static_mapping("Charizard", charizard_id);
    resolver.add_static_mapping("Mewtwo", mewtwo_id);

    // 瑙ｆ瀽鎵€鏈夎妭鐐?
    let (_, id1) = resolve_address(&resolver, "node:Pikachu").unwrap();
    let (_, id2) = resolve_address(&resolver, "node:Charizard").unwrap();
    let (_, id3) = resolve_address(&resolver, "node:Mewtwo").unwrap();

    assert_eq!(id1, pikachu_id);
    assert_eq!(id2, charizard_id);
    assert_eq!(id3, mewtwo_id);
}

#[test]
fn test_address_resolution_with_cache() {
    let resolver = MemAddressResolver::new();
    let id = NodeID::from_bytes(&[0xEF; 32]);

    // 鍏堢紦瀛?
    resolver.cache("Eevee", id, Duration::from_secs(300));

    // 瑙ｆ瀽搴旇浠庣紦瀛樺懡涓?
    let (_, resolved) = resolve_address(&resolver, "node:Eevee").unwrap();
    assert_eq!(resolved, id);

    // 鏈敞鍐岀殑鑺傜偣搴旇澶辫触
    assert!(resolve_address(&resolver, "node:Unknown").is_err());
}

#[test]
fn test_address_resolution_error_messages() {
    let resolver = MemAddressResolver::new();

    // 鏃犳晥鏍煎紡
    let err = resolve_address(&resolver, "192.168.1.1").unwrap_err();
    assert!(format!("{}", err).contains("invalid address format"));

    // 鏈煡鑺傜偣
    let err = resolve_address(&resolver, "node:NonExistent").unwrap_err();
    assert!(format!("{}", err).contains("unknown node"));
}

#[test]
fn test_cache_invalidation() {
    let resolver = MemAddressResolver::with_ttl(Duration::from_millis(50));
    let id = NodeID::from_bytes(&[0x11; 32]);

    resolver.cache("Pikachu", id, Duration::from_millis(50));

    // 绔嬪嵆搴旇鍙互鏌ュ埌
    assert!(resolver.get_cached("Pikachu").is_some());

    // 绛夊緟杩囨湡
    thread::sleep(Duration::from_millis(100));
    assert!(resolver.get_cached("Pikachu").is_none());

    // 缂撳瓨杩囨湡鍚庯紝闈欐€佹槧灏勪粛鐒舵湁鏁?
    resolver.add_static_mapping("Pikachu", id);
    let resolved = resolver.resolve("Pikachu").unwrap();
    assert_eq!(resolved, id);
}

// ===================== LAN 璺敱鍣ㄩ泦鎴愭祴璇?=====================

#[test]
fn test_lan_router_lifecycle() {
    let id = NodeID::from_bytes(&[0x22; 32]);
    let router = LanRouter::with_port("TestNode", id, 19900);

    // 鍚姩
    let result = router.start();
    assert!(result.is_ok());

    // 妫€鏌ョ姸鎬?
    let status = router.status();
    assert_eq!(status.node_status, NodeStatus::Online);
    assert_eq!(status.connection_type, ConnectionType::Lan);

    // 鍋滄
    router.stop();
    thread::sleep(Duration::from_millis(100));

    let status = router.status();
    assert_eq!(status.node_status, NodeStatus::Offline);
}

#[test]
fn test_lan_router_discovery() {
    let id1 = NodeID::from_bytes(&[0x31; 32]);
    let id2 = NodeID::from_bytes(&[0x32; 32]);

    // 涓や釜璺敱鍣ㄤ娇鐢ㄤ笉鍚岀鍙?
    let router_a = LanRouter::with_port("NodeA", id1, 19910);
    let router_b = LanRouter::with_port("NodeB", id2, 19911);

    router_a.start().unwrap();
    router_b.start().unwrap();

    // 涓や釜璺敱鍣ㄥ簲璇ラ兘鍦ㄧ嚎
    assert_eq!(router_a.status().node_status, NodeStatus::Online);
    assert_eq!(router_b.status().node_status, NodeStatus::Online);

    router_a.stop();
    router_b.stop();
    thread::sleep(Duration::from_millis(200));
}

#[test]
fn test_lan_router_send_invalid_address() {
    let id = NodeID::from_bytes(&[0x42; 32]);
    let router = LanRouter::with_port("TestNode", id, 19920);
    router.start().unwrap();

    // 鍙戦€佸埌鏃犳晥鍦板潃
    let result = router.send("invalid-address", b"test");
    assert!(result.is_err());
    match result {
        Err(RouterError::InvalidData(msg)) => {
            assert!(msg.contains("invalid address format"));
        }
        _ => panic!("expected InvalidData error"),
    }

    router.stop();
    thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_lan_router_send_unknown_node() {
    let id = NodeID::from_bytes(&[0x43; 32]);
    let router = LanRouter::with_port("TestNode", id, 19930);
    router.start().unwrap();

    // 鍙戦€佸埌涓嶅瓨鍦ㄧ殑鑺傜偣
    let result = router.send("node:Ghost", b"test data");
    assert!(result.is_err());
    match result {
        Err(RouterError::Unreachable(msg)) => {
            assert!(msg.contains("not found"));
        }
        _ => panic!("expected Unreachable error"),
    }

    router.stop();
    thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_lan_router_resolved_node_send() {
    let id = NodeID::from_bytes(&[0x44; 32]);
    let resolver = MemAddressResolver::new();
    resolver.add_static_mapping("TestPeer", NodeID::from_bytes(&[0x45; 32]));

    let router = LanRouter::with_port("TestNode", id, 19940);
    router.start().unwrap();

    // 鍗充娇鑺傜偣瀛樺湪浜庤В鏋愬櫒涓紝LAN 涓病鏈夎鑺傜偣鐨?UDP 鍦板潃涔熶細澶辫触
    let result = router.send("node:TestPeer", b"hello");
    assert!(result.is_err());

    router.stop();
    thread::sleep(Duration::from_millis(100));
}

// ===================== 绔埌绔泦鎴愭祴璇?=====================

#[test]
fn test_end_to_end_address_to_send() {
    // 妯℃嫙瀹屾暣娴佺▼锛氳В鏋愬湴鍧€ -> 鏌ユ壘鑺傜偣 -> 灏濊瘯鍙戦€?
    let resolver = MemAddressResolver::new();
    let local_id = NodeID::from_bytes(&[0x50; 32]);
    let peer_id = NodeID::from_bytes(&[0x51; 32]);

    resolver.add_static_mapping("Pikachu", local_id);
    resolver.add_static_mapping("Charizard", peer_id);

    // 瑙ｆ瀽鐩爣鍦板潃
    let (parsed, target_id) = resolve_address(&resolver, "node:Charizard").unwrap();
    assert_eq!(parsed.name, "Charizard");
    assert_eq!(target_id, peer_id);

    // 鍒涘缓璺敱鍣ㄥ苟灏濊瘯鍙戦€侊紙浼氬湪鍙戠幇闃舵澶辫触锛屽洜涓轰笉鍦ㄥ悓涓€ LAN锛?
    let router = LanRouter::with_port("Pikachu", local_id, 19950);
    router.start().unwrap();

    let result = router.send("node:Charizard", b"ping");
    // 棰勬湡澶辫触锛堜笉鍦?LAN 涓級
    assert!(result.is_err());

    router.stop();
    thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_router_trait_compliance() {
    let id = NodeID::from_bytes(&[0x60; 32]);
    let router = LanRouter::with_port("TraitTest", id, 19960);

    // 楠岃瘉 Router trait 鏂规硶
    let name: &str = Router::name(&router);
    assert_eq!(name, "TraitTest");

    let status: RouterStatus = Router::status(&router);
    assert_eq!(status.connection_type, ConnectionType::Lan);
    assert_eq!(status.known_nodes, 0);
    assert_eq!(status.active_routes, 0);
}

#[test]
fn test_concurrent_access() {
    let id = NodeID::from_bytes(&[0x70; 32]);
    let router = Arc::new(LanRouter::with_port("Concurrent", id, 19970));
    router.start().unwrap();

    let mut handles = vec![];

    // 骞跺彂璇诲彇鐘舵€?
    for _ in 0..5 {
        let r = router.clone();
        handles.push(thread::spawn(move || {
            let status = r.status();
            assert_eq!(status.connection_type, ConnectionType::Lan);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    router.stop();
    thread::sleep(Duration::from_millis(200));
}

// ===================== 鍗忚鍏煎鎬ф祴璇?=====================

#[test]
fn test_udp_message_type_display() {
    // 楠岃瘉娑堟伅绫诲瀷鍙互姝ｅ父浣跨敤
    assert_eq!(UdpMessageType::Discover.to_u8(), 0x01);
    assert_eq!(UdpMessageType::DiscoverReply.to_u8(), 0x02);
    assert_eq!(UdpMessageType::Data.to_u8(), 0x03);
    assert_eq!(UdpMessageType::Heartbeat.to_u8(), 0x04);

    assert!(UdpMessageType::from_u8(0x01).is_some());
    assert!(UdpMessageType::from_u8(0xFF).is_none());
}



#[test]
fn test_empty_data_transmission() {
    let id = NodeID::from_bytes(&[0xA0; 32]);
    let router = LanRouter::with_port("EmptyTest", id, 19980);
    router.start().unwrap();

    // 鍙戦€佺┖鏁版嵁搴旇鍙互鏋勫缓娑堟伅锛堣櫧鐒剁洰鏍囦笉瀛樺湪浼氬け璐ワ級
    let result = router.send("node:NonExistent", b"");
    assert!(result.is_err()); // 鐩爣涓嶅瓨鍦?

    router.stop();
    thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_multiple_resolvers_independence() {
    let resolver1 = MemAddressResolver::new();
    let resolver2 = MemAddressResolver::new();

    let id1 = NodeID::from_bytes(&[0xB1; 32]);
    let id2 = NodeID::from_bytes(&[0xB2; 32]);

    resolver1.add_static_mapping("Pikachu", id1);
    resolver2.add_static_mapping("Pikachu", id2); // 鍚屽悕涓嶅悓 ID

    let r1 = resolver1.resolve("Pikachu").unwrap();
    let r2 = resolver2.resolve("Pikachu").unwrap();

    assert_eq!(r1, id1);
    assert_eq!(r2, id2);
    assert_ne!(r1, r2);
}

#[test]
fn test_full_pipeline_address_resolve_and_router_send() {
    // 妯℃嫙 ll cmd node:Pikachu "uptime" 鐨勫畬鏁磋矾鐢辨祦绋?
    let resolver = MemAddressResolver::new();
    let local_id = NodeID::from_bytes(&[0xC0; 32]);
    let peer_id = NodeID::from_bytes(&[0xC1; 32]);

    resolver.add_static_mapping("Pikachu", local_id);
    resolver.add_static_mapping("Charizard", peer_id);

    let router = LanRouter::with_port("Pikachu", local_id, 19990);
    router.start().unwrap();

    // Step 1: 瑙ｆ瀽鍦板潃
    let addr = "node:Charizard";
    let (parsed, target_id) = resolve_address(&resolver, addr).unwrap();
    assert_eq!(parsed.name, "Charizard");
    assert_eq!(target_id, peer_id);

    // Step 2: 閫氳繃璺敱鍙戦€侊紙棰勬湡澶辫触锛屽洜涓?Charizard 涓嶅湪鍚屼竴 LAN锛?
    let cmd_data = b"uptime";
    let result = router.send(addr, cmd_data);
    assert!(result.is_err()); // 涓嶅湪 LAN 涓?

    // Step 3: 楠岃瘉璺敱鍣ㄧ姸鎬?
    let status = router.status();
    assert_eq!(status.connection_type, ConnectionType::Lan);
    assert_eq!(status.known_nodes, 0);

    router.stop();
    thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_lan_router_debug_format() {
    let id = NodeID::from_bytes(&[0xD0; 32]);
    let router = LanRouter::with_port("DebugTest", id, 19991);
    let debug_str = format!("{:?}", router);
    assert!(debug_str.contains("DebugTest"));
    assert!(debug_str.contains("19991"));
}


// ===================== DHT Mesh 集成测试 =====================

fn dht_make_id(byte: u8) -> NodeID {
    NodeID::from_bytes(&[byte; 32])
}

#[test]
fn test_dht_mesh_3_node_discovery() {
    let node_a = DhtManager::new(dht_make_id(0x01));
    let node_b = DhtManager::new(dht_make_id(0x02));
    let node_c = DhtManager::new(dht_make_id(0x03));

    // Step 1: A 发现 B
    assert_eq!(node_a.insert_node(dht_make_id(0x02), "10.0.0.2:9877".into()).unwrap(), true);
    assert_eq!(node_a.node_count(), 1);
    assert_eq!(node_b.node_count(), 0);

    // Step 2: B 发现 A 和 C
    assert_eq!(node_b.insert_node(dht_make_id(0x01), "10.0.0.1:9877".into()).unwrap(), true);
    assert_eq!(node_b.insert_node(dht_make_id(0x03), "10.0.0.3:9877".into()).unwrap(), true);
    assert_eq!(node_b.node_count(), 2);

    // Step 3: C 发现 B
    assert_eq!(node_c.insert_node(dht_make_id(0x02), "10.0.0.2:9877".into()).unwrap(), true);
    assert_eq!(node_c.node_count(), 1);

    // Step 4: B 查找 C — B knows A and C, returns both
    let nearest = node_b.find_node(&dht_make_id(0x03));
    assert!(nearest.len() >= 1, "B should return at least C");
    assert_eq!(nearest[0].0, dht_make_id(0x03), "nearest to C should be C itself");

    // Step 5: B 查找 A
    let nearest = node_b.find_node(&dht_make_id(0x01));
    assert!(nearest.len() >= 1, "B should return at least A");
    assert_eq!(nearest[0].0, dht_make_id(0x01), "nearest to A should be A itself");
}

#[test]
fn test_dht_mesh_5_node_full_mesh() {
    let ids: Vec<NodeID> = (1..=5).map(|b| dht_make_id(b)).collect();
    let nodes: Vec<DhtManager> = ids.iter().map(|id| DhtManager::new(*id)).collect();
    let addrs: Vec<String> = (1..=5).map(|i| format!("10.0.0.{}:9877", i)).collect();

    for (i, node) in nodes.iter().enumerate() {
        node.set_local_addr(addrs[i].clone());
    }

    for i in 0..5 {
        for j in 0..5 {
            if i != j {
                nodes[i].insert_node(ids[j], addrs[j].clone()).unwrap();
            }
        }
    }

    for (i, node) in nodes.iter().enumerate() {
        assert_eq!(node.node_count(), 4, "Node {} should know 4 others", i + 1);
    }

    for i in 0..5 {
        let nearest = nodes[i].find_node(&ids[(i + 2) % 5]);
        assert!(nearest.len() > 0, "Node {} should find nearest", i + 1);
        assert!(
            !nearest.iter().any(|(nid, _)| *nid == ids[i]),
            "Node {} should not find itself", i + 1
        );
    }

    // PUT/GET
    nodes[0].put_value(b"mesh-key", b"mesh-value".to_vec(), 3600);
    let get_result = nodes[0].get_value(b"mesh-key");
    assert!(get_result.is_some());
    assert_eq!(get_result.unwrap().value, b"mesh-value");
}

#[test]
fn test_dht_mesh_bucket_distribution() {
    let ids: Vec<NodeID> = (0..10).map(|b| dht_make_id(b * 25 + 1)).collect();
    let node = DhtManager::new(ids[0]);

    for i in 1..10 {
        node.insert_node(ids[i], format!("10.0.0.{}:9877", i)).unwrap();
    }
    assert_eq!(node.node_count(), 9);

    let mut seen = std::collections::HashSet::new();
    for i in 1..10 {
        seen.insert(node.bucket_index(&ids[i]));
    }
    assert!(seen.len() > 1, "Nodes should be in different buckets");
}

#[test]
fn test_dht_mesh_remove_and_rejoin() {
    let id_a = dht_make_id(0xA0);
    let id_b = dht_make_id(0xB0);
    let id_c = dht_make_id(0xC0);

    let node_a = DhtManager::new(id_a);

    node_a.insert_node(id_b, "10.0.0.2:9877".to_string()).unwrap();
    node_a.insert_node(id_c, "10.0.0.3:9877".to_string()).unwrap();
    assert_eq!(node_a.node_count(), 2);

    // C 离线
    let removed = node_a.remove_node(&id_c);
    assert!(removed.is_some());
    assert_eq!(node_a.node_count(), 1);

    // FIND_NODE 只返回 B（C 已不在）
    let nearest = node_a.find_node(&id_c);
    assert_eq!(nearest.len(), 1);
    assert_eq!(nearest[0].0, id_b);

    // C 重新上线
    node_a.insert_node(id_c, "10.0.0.3:9877".to_string()).unwrap();
    assert_eq!(node_a.node_count(), 2);

    let nearest = node_a.find_node(&id_c);
    assert!(nearest.iter().any(|(n, _)| *n == id_c),
            "Rejoined node should be findable");
}
