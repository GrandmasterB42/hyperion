//! Constructs for obtaining a player's skin.
use std::{sync::Arc, time::Duration};

use anyhow::{Context, bail};
use base64::{Engine as _, engine::general_purpose};
use bevy_ecs::{
    component::Component,
    lifecycle::HookContext,
    resource::Resource,
    world::{DeferredWorld, World},
};
use byteorder::NativeEndian;
use heed::{Database, Env, types};
use hyperion_data::LocalDb;
use hyperion_utils::{command_channel::CommandChannel, runtime::AsyncRuntime};
use rkyv::Archive;
use serde_json::Value;
use tokio::{
    sync::Semaphore,
    time::{MissedTickBehavior, interval},
};
use tracing::{debug, error, warn};
#[cfg(feature = "reflect")]
use {
    bevy_ecs::reflect::{ReflectComponent, ReflectResource},
    bevy_reflect::Reflect,
};

/// A handler for player skin operations
#[derive(Resource, Debug, Clone)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(opaque))]
pub struct SkinHandler {
    env: Env,
    skins: Database<types::U128<NativeEndian>, types::Bytes>,
}

#[derive(Debug)]
pub enum SkinCacheError {
    NotFound,
    Database(heed::Error),
}

impl From<heed::Error> for SkinCacheError {
    fn from(value: heed::Error) -> Self {
        Self::Database(value)
    }
}

impl SkinHandler {
    /// Creates a new [`SkinHandler`] from a given [`LocalDb`].
    pub fn new(db: &LocalDb) -> anyhow::Result<Self> {
        // We open the default unnamed database
        let skins = {
            let mut wtxn = db.write_txn()?;
            let db = db.create_database(&mut wtxn, Some("uuid-to-skins"))?;
            wtxn.commit()?;
            db
        };

        Ok(Self {
            env: db.env.clone(),
            skins,
        })
    }

    /// Finds [`PlayerSkinData`] by UUID.
    pub fn find(&self, uuid: &uuid::Uuid) -> Result<PlayerSkinData, SkinCacheError> {
        // We open a read transaction to check if those values are now available

        let uuid = uuid.as_u128();

        let rtxn = self.env.read_txn()?;
        let skin = self.skins.get(&rtxn, &uuid);

        let Some(skin) = skin? else {
            return Err(SkinCacheError::NotFound);
        };

        let skin = unsafe { rkyv::access_unchecked::<ArchivedPlayerSkinData>(skin) };
        let skin = rkyv::deserialize::<_, rkyv::rancor::Error>(skin).unwrap();
        Ok(skin)
    }

    /// Inserts a [`PlayerSkin`] into the database.
    pub fn insert(&self, uuid: &uuid::Uuid, skin: &PlayerSkinData) -> anyhow::Result<()> {
        let uuid = uuid.as_u128();

        let mut wtxn = self.env.write_txn()?;

        let skin = rkyv::to_bytes::<rkyv::rancor::Error>(skin).unwrap();

        self.skins.put(&mut wtxn, &uuid, &skin)?;
        wtxn.commit()?;

        Ok(())
    }
}

#[derive(
    Debug,
    Clone,
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize
)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
pub struct PlayerSkinData {
    /// The textures of the player skin, usually obtained from the [`MojangClient`] as a base64 string.
    pub textures: String,
    /// The signature of the player skin, usually obtained from the [`MojangClient`] as a base64 string.
    pub signature: String,
}

/// A signed player skin.
#[derive(Debug, Clone, Component)]
#[component(on_insert = resolve_skin)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub enum PlayerSkin {
    ToResolve,
    Resolved(PlayerSkinData),
    NotResolvable,
}

fn resolve_skin(mut world: DeferredWorld<'_>, ctx: HookContext) {
    let entity = ctx.entity;
    let uuid = world.get::<crate::Uuid>(entity).copied();

    let Some(uuid) = uuid else {
        error!("Failed to get UUID for entity {entity}");
        let mut skin = world
            .get_mut::<PlayerSkin>(entity)
            .expect("This was triggered from inside an insert hook, so it should be present");
        *skin = PlayerSkin::NotResolvable;
        return;
    };

    let mojang = world.resource::<MojangClient>().clone();
    let skins_collection = world.resource::<SkinHandler>().clone();
    let command_channel = world.resource::<CommandChannel>().clone();

    let runtime = world.resource::<AsyncRuntime>();
    runtime.spawn(async move {
        let fetch_result = PlayerSkin::from_uuid(&uuid, &mojang, &skins_collection).await;

        command_channel.push(move |world: &mut World| {
            let Some(mut skin) = world.get_mut::<PlayerSkin>(entity) else {
                warn!(
                    "failed to get entity after skin has been fetched (likely because the player \
                     has already left the server)"
                );
                return;
            };

            let s = match fetch_result {
                Ok(s) => s,
                Err(e) => {
                    error!("failed to get skin {e}. Using empty skin");
                    PlayerSkin::NotResolvable
                }
            };
            *skin = s;
        });
    });
}

impl PlayerSkin {
    /// Gets a skin from a Mojang UUID.
    ///
    /// # Arguments
    /// * `uuid` - A Mojang UUID.
    ///
    /// # Returns
    /// A `PlayerSkin` based on the UUID, or `None` if not found.
    pub async fn from_uuid(
        uuid: &uuid::Uuid,
        mojang: &MojangClient,
        skins: &SkinHandler,
    ) -> anyhow::Result<Self> {
        match skins.find(uuid) {
            Ok(skin) => {
                debug!("Returning cached skin");
                return Ok(PlayerSkin::Resolved(skin));
            }
            Err(e) => {
                error!("Error while tying to find skin in cache: {e:?}");
            }
        }

        debug!("player skin cache miss for {uuid}");

        let json_object = mojang.data_from_uuid(uuid).await?;
        let properties_array = json_object["properties"]
            .as_array()
            .with_context(|| format!("no properties on {json_object:?}"))?;
        for property_object in properties_array {
            let name = property_object["name"]
                .as_str()
                .with_context(|| format!("no name on {property_object:?}"))?;
            if name != "textures" {
                continue;
            }
            let textures = property_object["value"]
                .as_str()
                .with_context(|| format!("no value on {property_object:?}"))?;
            let signature = property_object["signature"]
                .as_str()
                .with_context(|| format!("no signature on {property_object:?}"))?;

            // Validate base64 encoding
            general_purpose::STANDARD
                .decode(textures)
                .context("invalid texture value")?;
            general_purpose::STANDARD
                .decode(signature)
                .context("invalid signature value")?;

            let data = PlayerSkinData {
                textures: textures.to_string(),
                signature: signature.to_string(),
            };
            skins.insert(uuid, &data)?;
            return Ok(PlayerSkin::Resolved(data));
        }
        Ok(PlayerSkin::NotResolvable)
    }
}

/// The API provider to use for Minecraft profile lookups
/// See [`MojangClient`].
#[derive(Clone, Copy)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
pub struct ApiProvider {
    username_base_url: &'static str,
    uuid_base_url: &'static str,
    max_requests: usize,
    interval: Duration,
}

impl ApiProvider {
    /// The matdoes.dev API mirror provider with higher rate limits
    pub const MAT_DOES_DEV: Self = Self {
        username_base_url: "https://mowojang.matdoes.dev/users/profiles/minecraft",
        uuid_base_url: "https://mowojang.matdoes.dev/session/minecraft/profile",
        max_requests: 10_000,
        interval: Duration::from_secs(1),
    };
    /// The official Mojang API provider
    pub const MOJANG: Self = Self {
        username_base_url: "https://api.mojang.com/users/profiles/minecraft",
        uuid_base_url: "https://sessionserver.mojang.com/session/minecraft/profile",
        max_requests: 600,
        interval: Duration::from_mins(10),
    };

    fn username_url(&self, username: &str) -> String {
        format!("{}/{username}", self.username_base_url)
    }

    fn uuid_url(&self, uuid: &uuid::Uuid) -> String {
        format!("{}/{uuid}?unsigned=false", self.uuid_base_url)
    }

    const fn max_requests(&self) -> usize {
        self.max_requests
    }

    const fn interval(&self) -> Duration {
        self.interval
    }
}

/// A client to interface with the Minecraft profile API.
///
/// Can use either the official Mojang API or [matdoes/mowojang](https://matdoes.dev/minecraft-uuids) as a data source.
/// This does not include caching, this should be done separately probably using [`hyperion_data::LocalDb`].
#[derive(Resource, Clone)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct MojangClient {
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    req: reqwest::Client,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    rate_limit: RateLimiter,
    provider: ApiProvider,
}

// Wrapper to allow reflect(ignore) on a semaphore
#[derive(Clone)]
struct RateLimiter(Arc<Semaphore>);

impl RateLimiter {
    fn new(permits: usize) -> Self {
        Self(Arc::new(Semaphore::new(permits)))
    }
}

impl std::default::Default for RateLimiter {
    fn default() -> Self {
        Self(Arc::new(Semaphore::new(0)))
    }
}

impl std::ops::Deref for RateLimiter {
    type Target = Arc<Semaphore>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MojangClient {
    #[must_use]
    pub fn new(runtime: &AsyncRuntime, provider: ApiProvider) -> Self {
        let rate_limit = RateLimiter::new(provider.max_requests());
        let interval_duration = provider.interval();

        runtime.spawn({
            let rate_limit = Arc::downgrade(&rate_limit);
            let max_requests = provider.max_requests();
            async move {
                let mut interval = interval(interval_duration);
                interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

                loop {
                    interval.tick().await;

                    let Some(rate_limit) = rate_limit.upgrade() else {
                        return;
                    };

                    let available = rate_limit.available_permits();
                    rate_limit.add_permits(max_requests - available);
                }
            }
        });

        Self {
            req: reqwest::Client::new(),
            rate_limit,
            provider,
        }
    }

    /// Gets a player's UUID from their username.
    pub async fn get_uuid(&self, username: &str) -> anyhow::Result<uuid::Uuid> {
        let url = self.provider.username_url(username);
        let json_object = self.response_raw(&url).await?;

        let id = json_object
            .get("id")
            .context("no id in json")?
            .as_str()
            .context("id is not a string")?;

        uuid::Uuid::parse_str(id).map_err(Into::into)
    }

    /// Gets a player's username from their UUID.
    pub async fn get_username(&self, uuid: &uuid::Uuid) -> anyhow::Result<String> {
        let url = self.provider.uuid_url(uuid);
        let json_object = self.response_raw(&url).await?;

        json_object
            .get("name")
            .context("no name in json")?
            .as_str()
            .map(String::from)
            .context("Username not found")
    }

    /// Gets player data from their UUID.
    pub async fn data_from_uuid(&self, uuid: &uuid::Uuid) -> anyhow::Result<Value> {
        let url = self.provider.uuid_url(uuid);
        self.response_raw(&url).await
    }

    /// Gets player data from their username.
    pub async fn data_from_username(&self, username: &str) -> anyhow::Result<Value> {
        let url = self.provider.username_url(username);
        self.response_raw(&url).await
    }

    async fn response_raw(&self, url: &str) -> anyhow::Result<Value> {
        self.rate_limit
            .acquire()
            .await
            .expect("semaphore is never closed")
            .forget();

        if self.rate_limit.available_permits() == 0 {
            warn!(
                "rate limiting will be applied: {} requests have been sent in the past {:?} \
                 interval",
                self.provider.max_requests(),
                self.provider.interval()
            );
        }

        let response = self.req.get(url).send().await?;

        if response.status().is_success() {
            let body = response.text().await?;
            let json_object = serde_json::from_str::<Value>(&body)
                .with_context(|| format!("failed to parse json from response: {body:?}"))?;

            if let Some(error) = json_object.get("error") {
                bail!("API Error: {}", error.as_str().unwrap_or("Unknown error"));
            }
            Ok(json_object)
        } else {
            bail!("Failed to retrieve data from API");
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::print_stdout, reason = "these are tests")]
mod tests {
    use std::str::FromStr;

    use hyperion_utils::runtime::AsyncRuntime;

    use crate::skin::{ApiProvider, MojangClient};

    #[test]
    fn test_get_uuid() {
        let tasks = AsyncRuntime::new();
        let mojang = MojangClient::new(&tasks, ApiProvider::MAT_DOES_DEV);

        let uuid = tasks.block_on(mojang.get_uuid("Emerald_Explorer")).unwrap();
        let expected = uuid::Uuid::from_str("86271406-1188-44a5-8496-7af10c906204").unwrap();
        assert_eq!(uuid, expected);
    }

    #[test]
    fn test_get_username() {
        let tasks = AsyncRuntime::new();
        let mojang = MojangClient::new(&tasks, ApiProvider::MAT_DOES_DEV);

        let username = tasks
            .block_on(mojang.get_username(
                &uuid::Uuid::from_str("86271406-1188-44a5-8496-7af10c906204").unwrap(),
            ))
            .unwrap();
        assert_eq!(username, "Emerald_Explorer");
    }

    #[test]
    fn test_retrieve_username() {
        let tasks = AsyncRuntime::new();
        let mojang = MojangClient::new(&tasks, ApiProvider::MAT_DOES_DEV);

        let res = tasks
            .block_on(mojang.data_from_uuid(
                &uuid::Uuid::from_str("86271406-1188-44a5-8496-7af10c906204").unwrap(),
            ))
            .unwrap();

        // {
        //   "id": "86271406118844a584967af10c906204",
        //   "name": "Emerald_Explorer",
        //   "profileActions": [],
        //   "properties": [
        //     {
        //       "name": "textures",
        //       "signature": "vSdWxKrUendEP7rapc8Kw2RP6oxWH75CaDrdLXIZlXRmM3+lIYbxaUr8feA0gtZTdiJPTA9GstQHr6mIz1Ap2gm6pd50LVj22yRA1e1qgmAEq8L6EZj7MPnN/kgvWnUj2XFdhP1TsENi3ekvDLHuvRSdeOKgdew3u6/3h6DLAZp/6w2Z89wRJRytWDrSxm3YrPJpGyUA0DjYkoKlCi2n4fd6iTxGzPCnN0gi/y1ewEGbz9rVSsN9EX+tecACl/W4PAOo2wtSEDBziHOMmAEFunmzVReo24XNTTTqQNf6wywAFbXRPaSsRayYrc1vwPXNj4mZwep1LbP8/qQsefjNi3olBmXLxnyxD62Zyx2ZK3NBD1Qbc40PiM6qhpuoQxUgPQHTxL3XazzatH4sQv11rWxLYJhppVsWxUNMy696e5JK7oVtUgSSPbqVjQYdPpn/z22ZzwXh3Y0vkbxfTZ8aZSxEYhJzUtlDNFKcaWEPzuohBsUPELISELLWmL46Rue96gR2lUxdStlUR15L4XZ3cpINTCLj1AQdl2q6mP0T7ooG/Cvri0qKtZ/RuJ3HUZMFfZB6SQ5LGbpwfwPwCWxgYkpwhIUNvLBaEQQNDXELmYgomLE1rd/q6FdM4HaSYCqxBgMyQPzkeOkrZ4k9pBaU16rRWwkCvek4Evdz2L5cpMo=",
        //       "value": "ewogICJ0aW1lc3RhbXAiIDogMTczMDY0Mjc1NjU0OCwKICAicHJvZmlsZUlkIiA6ICI4NjI3MTQwNjExODg0NGE1ODQ5NjdhZjEwYzkwNjIwNCIsCiAgInByb2ZpbGVOYW1lIiA6ICJFbWVyYWxkX0V4cGxvcmVyIiwKICAic2lnbmF0dXJlUmVxdWlyZWQiIDogdHJ1ZSwKICAidGV4dHVyZXMiIDogewogICAgIlNLSU4iIDogewogICAgICAidXJsIiA6ICJodHRwOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzE1MTBlM2VlM2YwZThkNTJhMGUxZjMzY2UwYmJiZTRhZWE4Yjg4MzhjOWJkYzQ5NjEzNDI2ZWJhYjYxNGE2ODMiCiAgICB9CiAgfQp9"
        //     }
        //   ]
        // }

        // {
        //   "timestamp" : 1730642756548,
        //   "profileId" : "86271406118844a584967af10c906204",
        //   "profileName" : "Emerald_Explorer",
        //   "signatureRequired" : true,
        //   "textures" : {
        //     "SKIN" : {
        //       "url" : "http://textures.minecraft.net/texture/1510e3ee3f0e8d52a0e1f33ce0bbbe4aea8b8838c9bdc49613426ebab614a683"
        //     }
        //   }
        // }⏎

        let pretty = serde_json::to_string_pretty(&res).unwrap();
        println!("{pretty}");
    }
}
