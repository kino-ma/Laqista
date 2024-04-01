extern crate test;

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use test::Bencher;

    use crate::{
        app::{self, proto::HelloRequest},
        proto::{self, DeployRequest, LookupRequest},
    };

    use super::*;

    #[bench]
    fn bench_scheduled(b: &mut Bencher) {
        let (client, deploy_response) = block_on(async {
            let mut client =
                proto::scheduler_client::SchedulerClient::connect("http://127.0.0.1:50051")
                    .await
                    .expect("failed to connect to the server");

            let request = DeployRequest {
                source: "https://github.com/kino-ma/MLess".to_owned(),
                authoritative: true,
            };

            let deployment = client.deploy(request).await.expect("failed to deploy");

            (client, deployment)
        });

        let deployment = deploy_response.into_inner().deployment.unwrap();
        let deployment_id = deployment.id;

        b.iter(|| async {
            let request = LookupRequest {
                deployment_id: deployment_id.clone(),
                qos: None,
            };

            let resp = client.clone().lookup(request).await.unwrap().into_inner();
            let server = resp.server.unwrap();
            let addr = server.addr.clone();

            let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
                .await
                .unwrap();
            let request = HelloRequest {
                name: "MLess benchamrk".to_owned(),
            };
            app_client.say_hello(request).await.unwrap();
        })
    }
}
