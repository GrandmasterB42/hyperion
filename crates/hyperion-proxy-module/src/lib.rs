use std::{net::SocketAddr, path::Path};

use bevy_app::{App, Plugin};
use bevy_ecs::{event::Event, observer::On, system::Res};
use hyperion_utils::runtime::AsyncRuntime;
use tokio::net::TcpListener;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectEvent, bevy_reflect::Reflect};

pub struct HyperionProxyPlugin;

#[derive(Event)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Event))]
pub struct SetProxyAddress {
    pub proxy: String,
    pub server: String,
}

impl Default for SetProxyAddress {
    fn default() -> Self {
        Self {
            proxy: "0.0.0.0:25565".to_string(),
            server: "127.0.0.1:35565".to_string(),
        }
    }
}

impl Plugin for HyperionProxyPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(update_proxy_address);
    }
}

fn update_proxy_address(
    set_proxy_adress: On<'_, '_, SetProxyAddress>,
    runtime: Res<'_, AsyncRuntime>,
) {
    let proxy = set_proxy_adress.proxy.clone();
    let server = set_proxy_adress.server.clone();

    runtime.spawn(async move {
        let listener = TcpListener::bind(&proxy).await.unwrap();
        tracing::info!("Listening on {proxy}");

        let addr: SocketAddr = tokio::net::lookup_host(&server)
            .await
            .unwrap()
            .next()
            .unwrap();

        // TODO: Why are the paths hardcoded?
        hyperion_proxy::run_proxy(
            listener,
            addr,
            server.clone(),
            Path::new("root_ca.crt"),
            Path::new("proxy.crt"),
            Path::new("proxy_private_key.pem"),
        )
        .await
        .unwrap();
    });
}
