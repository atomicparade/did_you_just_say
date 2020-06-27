use dotenv::dotenv;
use log;

use std::sync::Arc;
use log::{error, info};
use std::{env, process};

use serenity::client::Client;
use serenity::client::bridge::gateway::ShardManager;
use serenity::model::prelude::{Message, Ready};
use serenity::prelude::{Context, EventHandler, Mutex, TypeMapKey};

struct ShardManagerKey;

impl TypeMapKey for ShardManagerKey {
    type Value = Arc<Mutex<ShardManager>>;
}

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        info!(
            "Connected as {}#{}",
            ready.user.name, ready.user.discriminator
        );
    }

    fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "test" {
            msg.channel_id.say(&ctx.http, "Test successful").ok();
        } else if msg.content == "quit" {
            info!(
                "Quitting at the request of {}#{}",
                msg.author.name, msg.author.discriminator
            );

            let data = ctx.data.read();

            let shard_manager = match data.get::<ShardManagerKey>() {
                Some(shard_manager) => shard_manager,
                None => {
                    process::exit(1);
                }
            };

            shard_manager.lock().shutdown_all();
        }
    }
}

fn main() {
    dotenv().ok();
    env_logger::init();

    let discord_bot_token = match env::var("DISCORD_BOT_TOKEN") {
        Ok(token) => token,
        Err(_) => {
            error!("DISCORD_BOT_TOKEN is missing");
            process::exit(1);
        }
    };

    info!("Connecting");

    let mut client = match Client::new(&discord_bot_token, Handler) {
        Ok(client) => client,
        Err(reason) => {
            error!("Unable to create client: {:?}", reason);
            process::exit(1);
        }
    };

    // Store the ShardManager in the client's data in order to allow event
    // handler methods to access it
    {
        let mut data = client.data.write();
        data.insert::<ShardManagerKey>(Arc::clone(&client.shard_manager));
    }

    if let Err(reason) = client.start() {
        error!("Unable to start client: {:?}", reason);
        process::exit(1);
    }
}
