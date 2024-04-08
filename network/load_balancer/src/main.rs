use pingora::prelude::*;
use pingora_core::server::configuration::Opt;
use std::sync::Arc;
use std::io::Read;
//use clap::Parser;
use async_trait::async_trait;
use structopt::StructOpt;

pub struct LB(Arc<LoadBalancer<RoundRobin>>);

#[async_trait]
impl ProxyHttp for LB {
    type CTX = ();
    fn new_ctx(&self) -> () {
        ()
    }

    async fn upstream_peer(&self, _session: &mut Session, _ctx: &mut ()) -> Result<Box<HttpPeer>> {
        let upstream = self.0
            .select(b"", 256)  // hash doesn't matter for round robin
            .unwrap();

        //println!("upstream peer is : {upstream:?}");

        // set use_tls to true and set SNI to one.one.one.one
        //let peer = Box::new(HttpPeer::new(upstream, true, "one.one.one.one".to_string()));
        let peer = Box::new(HttpPeer::new(upstream, false, "".to_string()));
        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        upstream_request.insert_header("Host", "one.one.one.one").unwrap();
        Ok(())
    }
}

//#[derive(Parser)]
//struct Opts {
//    /// LoadBalancer upstreams
//    #[arg(short, long, required = true)]
//    upstream: Vec<String>,
//}

fn main() {
    //let opts = Opts::parse();
    // read command line arguments
    let opt = Opt::from_args();
    let mut my_server = Server::new(Some(opt)).unwrap();

    my_server.bootstrap();

    //println!("Please input backend: ");
    //let mut upstream1 = String::new();
    //std::io::stdin().read_to_string(&mut upstream1).unwrap();
    //let mut upstream2 = String::new();
    //std::io::stdin().read_to_string(&mut upstream2).unwrap();
    let upstreams =
        LoadBalancer::try_from_iter(["169.254.0.17:80", "169.254.0.18:80"]).unwrap();

    let mut lb = http_proxy_service(&my_server.configuration, LB(Arc::new(upstreams)));
    lb.add_tcp("0.0.0.0:6188");

    my_server.add_service(lb);

    my_server.run_forever();
}
