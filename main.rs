use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use hyper::{Body, Method, Request, Response, Server, StatusCode};
use hyper::service::{make_service_fn, service_fn};

type BoxFut = Box<dyn Future<Item = Response<Body>, Error = hyper::Error> + Send>;

struct Firewall {
    rules: Arc<Mutex<HashMap<IpAddr, u64>>>,
    threshold: u64,
    ban_duration: Duration,
}

impl Firewall {
    fn new(threshold: u64, ban_duration: Duration) -> Self {
        Self {
            rules: Arc::new(Mutex::new(HashMap::new())),
            threshold,
            ban_duration,
        }
    }

    fn allow(&self, ip: IpAddr) -> BoxFut {
        let mut rules = self.rules.lock().unwrap();
        let entry = rules.entry(ip).or_insert(0);
        *entry += 1;

        if *entry > self.threshold {
            println!("Blocking IP {} for {} seconds", ip, self.ban_duration.as_secs());
            thread::spawn(move || {
                thread::sleep(self.ban_duration);
                let mut rules = self.rules.lock().unwrap();
                rules.remove(&ip);
            });

            Box::new(future::ok(
                Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .body(Body::empty())
                    .unwrap(),
            ))
        } else {
            println!("Allowing IP {}", ip);
            Box::new(future::ok(Response::new(Body::empty())))
        }
    }
}

fn main() {
    let firewall = Firewall::new(10, Duration::from_secs(60));

    let make_service = make_service_fn(|_conn| {
        let firewall = firewall.clone();
        let fw = move || {
            let firewall = firewall.clone();
            service_fn(move |req: Request<Body>| -> BoxFut {
                let ip: IpAddr = req
                    .remote_addr()
                    .ip()
                    .clone()
                    .into();
                firewall.allow(ip)
            })
        };
        future::ok(fw())
    });

    let addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 25565);
    let server = Server::bind(&addr).serve(make_service);

    println!("Listening on http://{}", addr);
    hyper::rt::run(server.map_err(|e| {
        println!("server error: {}", e);
    }));
}
