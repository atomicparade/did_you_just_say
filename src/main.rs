use dotenv::dotenv;
use log::{debug, error, info};
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

struct Command<'a> {
    entire: &'a str,
    first_word: &'a str,
    rest: &'a str,
}

fn is_command<'a>(ctx: &Context, msg: &'a Message) -> Option<Command<'a>> {
    // Check whether the message begins with a mention of the bot
    let data = ctx.data.read();
    let bot_user_id = data
        .get::<BotUserIdKey>()
        .expect("Unable to retrieve bot user ID");

    let re_pattern = format!(r"<@!?{}>\s*(\S*)\s*((?s).*)", bot_user_id);
    let re_command =
        Regex::new(&re_pattern).expect("Unable to create command matching pattern with bot ID");

    if let Some(captures) = re_command.captures(&msg.content) {
        return Some(Command {
            entire: captures.get(0).expect("Unable to extract entire command text from message beginning with bot ID").as_str(),
            first_word: captures.get(1).expect("Unable to extract first word of command text from message beginning with bot ID").as_str(),
            rest: captures.get(2).expect("Unable to extract rest of command text from message beginning with bot ID").as_str(),
        });
    }

    // Check whether this is a DM
    if let Some(channel) = msg.channel(ctx) {
        if channel.private().is_some() {
            let re_pattern = r"(\S*)\s*((?s).*)";
            let re_command = Regex::new(re_pattern)
                .expect("Unable to create command matching pattern without bot ID");

            if let Some(captures) = re_command.captures(&msg.content) {
                return Some(Command {
                    entire: captures.get(0).expect("Unable to extract entire command text from message beginning without bot ID").as_str(),
                    first_word: captures.get(1).expect("Unable to extract first word of command text from message beginning without bot ID").as_str(),
                    rest: captures.get(2).expect("Unable to extract rest of command text from message beginning without bot ID").as_str(),
                });
            }
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
            Some(command) => command,
            None => return,
        };

        debug!(
            "Received command; first word: \"{}\", rest: \"{}\"",
            command.first_word, command.rest
        );

        match command.first_word.to_lowercase().as_str() {
            "auth" => {
                // TODO
            }
            "quit" => {
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
            }
            _ => {
                info!("Creating image for string {}", command.entire);
                // TODO
            }
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
