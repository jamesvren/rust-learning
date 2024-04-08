use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UdpSocket;
use pnet::datalink;
use std::path::Path;
use ovsdb::Client;
use ovsdb::protocol::method::Operation;

#[tokio::main]
async fn main() {
    let client = Client::connect_unix(Path::new("/var/run/openvswitch/db.sock"))
        .await
        .unwrap();
    let dbs = client.list_databases().await.unwrap();
    println!("databases: {:#?}", dbs);
    for db in dbs.as_slice() {
        // it will hange when getting schema
        //let schema = client.get_schema(db).await.unwrap();
        //println!("{db}'s schema {schema:#?}");
        let oper = Operation::Select {
            table: "Bridge".to_string(),
            clauses: vec![],
        };
        let trans = client.transact::<_, serde_json::Value>(db, vec![oper])
            .await
            .unwrap();
        println!("@@ {:#}", trans);
        for bridge in trans[0]["rows"].as_array().unwrap() {
            println!("Got: {}", bridge["name"]);
        }
    }
    //let interfaces = netdev::get_interfaces();
    //for interface in interfaces {
    //    println!("{:#?}", interface);
    //}
}
