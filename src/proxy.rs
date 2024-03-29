use axum::routing::{any, MethodRouter};

pub fn create_reverse_proxy(package: &str, addr: &str) -> MethodRouter {
    let package = package.to_owned();
    let addr = addr.to_owned();
    any(|| async move { format!("hello from handler!\n{:?} -> {:?}", package, addr) })
}
