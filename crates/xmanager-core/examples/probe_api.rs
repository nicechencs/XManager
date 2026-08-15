//! Live probe: whoami + tweet lookup. Does not delete.
//! cargo run -p xmanager-core --example probe_api -- 2083205060025319920

use std::env;
use xmanager_core::{Settings, XClient};

fn main() {
    let ids: Vec<String> = env::args().skip(1).collect();
    let settings = match Settings::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("settings: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = settings.require_oauth1() {
        eprintln!("creds: {e}");
        std::process::exit(2);
    }
    let client = match XClient::new(settings) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("client: {e}");
            std::process::exit(2);
        }
    };
    match client.get_me() {
        Ok(u) => println!("whoami ok id={} username={}", u.id, u.username),
        Err(e) => {
            eprintln!("whoami fail: {e}");
            std::process::exit(3);
        }
    }
    if ids.is_empty() {
        println!("no ids; skip lookup");
        return;
    }
    match client.lookup_tweets(&ids) {
        Ok(lookup) => {
            println!(
                "lookup ok found={} missing={} failed={}",
                lookup.found.len(),
                lookup.missing.len(),
                lookup.failed.len()
            );
            for t in &lookup.found {
                println!("  found {} views={}", t.id, t.views());
            }
            for id in &lookup.missing {
                println!("  missing {id}");
            }
            for (id, err) in &lookup.failed {
                println!("  failed {id}: {err}");
            }
        }
        Err(e) => {
            eprintln!("lookup fail: {e}");
            std::process::exit(3);
        }
    }
}
