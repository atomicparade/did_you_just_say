use dotenv::dotenv;
use log::{error, info};
use regex::Regex;
use std::sync::Arc;
use std::{env, process};

use serenity::client::bridge::gateway::ShardManager;
use serenity::client::Client;
use serenity::model::prelude::{Message, Ready};
use serenity::prelude::{Context, EventHandler, Mutex, TypeMapKey};

struct BotUserIdKey;

impl TypeMapKey for BotUserIdKey {
    type Value = u64;
}

struct ShardManagerKey;

impl TypeMapKey for ShardManagerKey {
    type Value = Arc<Mutex<ShardManager>>;
}

struct Command<'a>(&'a str);

fn is_command<'a>(ctx: &Context, msg: &'a Message) -> Option<Command<'a>> {
    // Check whether the message begins with a mention of the bot
    let data = ctx.data.read();
    let bot_user_id = data
        .get::<BotUserIdKey>()
        .expect("Unable to retrieve bot user ID");

    let re_pattern = format!(r"<@!?{}>\s*((?s).*)", bot_user_id);
    let re_command = Regex::new(&re_pattern).expect("Unable to create command matching pattern");

    if let Some(captures) = re_command.captures(&msg.content) {
        return Some(Command(
            captures
                .get(1)
                .expect("Unable to extract command text from message containing command")
                .as_str(),
        ));
    }

    // Check whether this is a DM
    if let Some(channel) = msg.channel(ctx) {
        if channel.private().is_some() {
            return Some(Command(&msg.content));
        }
    }

    return None;
}

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        info!(
            "Connected as {}#{}",
            ready.user.name, ready.user.discriminator
        );

        let mut data = ctx.data.write();
        data.insert::<BotUserIdKey>(ready.user.id.0);
    }

    fn message(&self, ctx: Context, msg: Message) {
        let command = match is_command(&ctx, &msg) {
            Some(Command(command)) => command,
            None => return,
        };

        if command.to_lowercase() == "quit" {
            // TODO: Validate the user's authority

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
        } else {
            info!("Creating image for string {}", command);
            // TODO
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
