use dotenv::dotenv;
use image::DynamicImage;
use log::{debug, error, info, warn};
use regex::Regex;
use std::fs::remove_file;
use std::sync::Arc;
use std::{env, process};
use tempfile::tempdir;

use serenity::client::bridge::gateway::ShardManager;
use serenity::client::Client;
use serenity::model::prelude::{Message, Ready};
use serenity::prelude::{Context, EventHandler, Mutex, TypeMapKey};

const BASE_IMAGE_PATH: &str = "did_you_just_say.png";

struct BotSettings {
    id: Option<u64>,
    admin_password: Option<String>,
    admin_ids: Vec<u64>,
}

struct BotSettingsKey;

impl TypeMapKey for BotSettingsKey {
    type Value = BotSettings;
}

struct BaseImageKey;

impl TypeMapKey for BaseImageKey {
    type Value = DynamicImage;
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
    let settings = data
        .get::<BotSettingsKey>()
        .expect("is_command(): Unable to retrieve bot settings");
    let bot_user_id = settings
        .id
        .expect("is_command(): Unable to retrieve bot user ID");

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
    if let Some(channel) = msg.channel(&ctx) {
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
        let settings = data
            .get_mut::<BotSettingsKey>()
            .expect("ready(): Unable to retrieve bot settings");
        settings.id = Some(ready.user.id.0);
    }

    fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

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
                // Ensure that this is a private channel
                if let Some(channel) = msg.channel(&ctx) {
                    if channel.private().is_none() {
                        return;
                    }
                } else {
                    return;
                }

                // If the user isn't already in the admin list, attempt to
                // verify
                let mut data = ctx.data.write();
                let settings = data
                    .get_mut::<BotSettingsKey>()
                    .expect("Command quit: Unable to retrieve bot settings");

                if settings.admin_ids.contains(msg.author.id.as_u64()) {
                    msg.channel_id.say(&ctx, "You are already authorized.").ok();
                }

                if let Some(admin_password) = &settings.admin_password {
                    if admin_password == command.rest {
                        info!(
                            "User sucessfully authorized as admin: {}#{}",
                            msg.author.name, msg.author.discriminator
                        );

                        settings.admin_ids.push(msg.author.id.0);

                        msg.channel_id.say(&ctx, "Successfully authorized.").ok();
                    } else {
                        info!(
                            "User failed attempt to authorize as admin: {}#{}",
                            msg.author.name, msg.author.discriminator
                        );
                    }
                }
            }
            "quit" => {
                let data = ctx.data.read();
                let settings = data
                    .get::<BotSettingsKey>()
                    .expect("Command quit: Unable to retrieve bot settings");

                if !settings.admin_ids.contains(msg.author.id.as_u64()) {
                    return;
                }

                info!(
                    "User requested quit: {}#{}",
                    msg.author.name, msg.author.discriminator
                );

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

                let data = ctx.data.read();
                let base_image = data
                    .get::<BaseImageKey>()
                    .expect("Command create_image: Unable to retrieve base image");

                static GENERATED_IMAGE_FILENAME: &str = "did_you_just_say.png";
                let temp_dir =
                    tempdir().expect("Command create_image: Failed to create temporary directory");

                let file_path = format!(
                    "{}/{}",
                    temp_dir.path().to_str().expect(
                        "Command create_image: Failed to retrieve temporary directory path"
                    ),
                    GENERATED_IMAGE_FILENAME
                );

                // TODO: Write command.rest to the image

                match base_image.save(&file_path) {
                    Ok(_) => {
                        msg.channel_id
                            .send_files(&ctx, vec![file_path.as_str()], |m| m)
                            .ok();

                        if let Err(reason) = remove_file(&file_path) {
                            warn!("Command create_image: Temporary file \"{}\" could not be deleted: {:?}", file_path, reason);
                        }
                    }
                    Err(reason) => {
                        msg.channel_id
                            .say(&ctx, "Sorry, something went wrong! Maybe try again?")
                            .ok();

                        warn!(
                            "Command create_image: Failed to save image to \"{}\": {:?}",
                            file_path, reason
                        );
                    }
                }

                // temp_dir falls out of scope and is automatically deleted
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

    let base_image = match image::open(BASE_IMAGE_PATH) {
        Ok(image) => image,
        Err(reason) => {
            error!("Unable to open image {}: {:?}", BASE_IMAGE_PATH, reason);
            process::exit(1);
        }
    };

    let bot_admin_password = match env::var("BOT_ADMIN_PASSWORD") {
        Ok(mut password) => {
            password = password.trim().to_string();

            if password.is_empty() {
                None
            } else {
                Some(password)
            }
        }
        Err(_) => None,
    };

    if bot_admin_password.is_none() {
        warn!("No bot admin password specified");
    }

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
        data.insert::<BotSettingsKey>(BotSettings {
            id: None,
            admin_password: bot_admin_password,
            admin_ids: Vec::<u64>::new(),
        });
        data.insert::<BaseImageKey>(base_image);
    }

    if let Err(reason) = client.start() {
        error!("Unable to start client: {:?}", reason);
        process::exit(1);
    }
}
