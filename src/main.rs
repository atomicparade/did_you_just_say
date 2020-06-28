use dotenv::dotenv;
use image::{Pixel, RgbaImage};
use imageproc::drawing;
use log::{debug, error, info, warn};
use regex::Regex;
use rusttype::{Font, Point, Scale};
use std::collections::HashMap;
use std::fs::{read_to_string, remove_file, File};
use std::io::Read;
use std::sync::Arc;
use std::{env, process};
use tempfile::tempdir;
use yaml_rust::yaml::Yaml;
use yaml_rust::YamlLoader;

use serenity::client::bridge::gateway::ShardManager;
use serenity::client::Client;
use serenity::model::prelude::{Message, Ready};
use serenity::prelude::{Context, EventHandler, Mutex, TypeMapKey};

struct BotSettings {
    id: Option<u64>,
    admin_password: Option<String>,
    admin_ids: Vec<u64>,
}

struct BotSettingsKey;

impl TypeMapKey for BotSettingsKey {
    type Value = BotSettings;
}

struct FontsKey;

impl TypeMapKey for FontsKey {
    type Value = HashMap<String, Font<'static>>;
}

struct Meme {
    image: RgbaImage,
    font: String,
    scale: Scale,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    center: Point<u32>,
    text_prefix: String,
    text_suffix: String,
    command: String,
    is_default: bool,
}

struct MemesKey;

impl TypeMapKey for MemesKey {
    type Value = Vec<Meme>;
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
            entire:
                &msg.content[captures.get(1).expect("Unable to extract entire command text from message beginning with bot ID").start()..],
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

fn load_font(filename: &str) -> Result<Font<'static>, String> {
    let mut font_file = match File::open(&filename) {
        Ok(file) => file,
        Err(reason) => {
            return Err(format!("Unable to open file \"{}\": {}", filename, reason));
        }
    };

    let mut buffer = Vec::new();

    if let Err(reason) = font_file.read_to_end(&mut buffer) {
        return Err(format!("Unable to read file \"{}\": {}", filename, reason));
    }

    let font = match Font::from_bytes(buffer) {
        Ok(font) => font,
        Err(reason) => {
            return Err(format!("Unable to open font \"{}\": {}", filename, reason));
        }
    };

    Ok(font)
}

fn load_image(filename: &str) -> Result<RgbaImage, String> {
    let image = match image::open(filename) {
        Ok(image) => image.to_rgba(),
        Err(reason) => {
            return Err(format!("Unable to open image {}: {}", filename, reason));
        }
    };

    return Ok(image);
}

fn load_memes(filename: &str) -> (HashMap<String, Font<'static>>, Vec<Meme>) {
    let mut fonts = HashMap::<String, Font<'static>>::new();
    let mut memes = Vec::<Meme>::new();

    let config = match read_to_string(&filename) {
        Ok(contents) => contents,
        Err(reason) => {
            error!("Unable to read config file \"{}\": {}", filename, reason);
            process::exit(1);
        }
    };

    let yaml = match YamlLoader::load_from_str(&config) {
        Ok(yaml) => yaml,
        Err(reason) => {
            error!("Unable to parse config file \"{}\": {}", filename, reason);
            process::exit(1);
        }
    };

    let yaml = match yaml.first() {
        Some(yaml) => yaml,
        _ => {
            error!("Empty config file");
            process::exit(1);
        }
    };

    if let Yaml::Array(meme_sections) = yaml {
        for meme_section in meme_sections {
            if let Yaml::Hash(hash) = meme_section {
                let mut read_image_filename: Option<String> = None;
                let mut read_font_filename: Option<String> = None;
                let mut read_font_size: Option<u32> = None;
                let mut read_left: Option<u32> = None;
                let mut read_top: Option<u32> = None;
                let mut read_right: Option<u32> = None;
                let mut read_bottom: Option<u32> = None;
                let mut read_text_prefix: Option<String> = None;
                let mut read_text_suffix: Option<String> = None;
                let mut read_command: Option<String> = None;
                let mut read_is_default: Option<bool> = None;

                for (key, value) in hash {
                    let key = match key {
                        Yaml::String(key) => key,
                        unknown_key => {
                            warn!(
                                "Config contains invalid non-string key \"{:?}\"",
                                unknown_key
                            );
                            continue;
                        }
                    };

                    match key.as_str() {
                        "filename" => {
                            if let Yaml::String(image_filename) = value {
                                read_image_filename = Some(image_filename.clone());
                            } else {
                                warn!(
                                    "Config contains invalid value for image filename \"{:?}\"",
                                    value
                                );
                            }
                        }
                        "font" => {
                            if let Yaml::String(font_filename) = value {
                                read_font_filename = Some(font_filename.clone());
                            } else {
                                warn!(
                                    "Config contains invalid value for font filename \"{:?}\"",
                                    value
                                );
                            }
                        }
                        "font_size" => {
                            let mut valid_value_found = false;

                            if let Yaml::Integer(font_size) = value {
                                if *font_size > 0 {
                                    read_font_size = Some(*font_size as u32);
                                    valid_value_found = true;
                                }
                            }

                            if !valid_value_found {
                                warn!(
                                    "Config contains invalid value for font_size: \"{:?}\"",
                                    value
                                );
                            }
                        }
                        "left" => {
                            let mut valid_value_found = false;

                            if let Yaml::Integer(left) = value {
                                if *left > 0 {
                                    read_left = Some(*left as u32);
                                    valid_value_found = true;
                                }
                            }

                            if !valid_value_found {
                                warn!("Config contains invalid value for left: \"{:?}\"", value);
                            }
                        }
                        "top" => {
                            let mut valid_value_found = false;

                            if let Yaml::Integer(top) = value {
                                if *top > 0 {
                                    read_top = Some(*top as u32);
                                    valid_value_found = true;
                                }
                            }

                            if !valid_value_found {
                                warn!("Config contains invalid value for top: \"{:?}\"", value);
                            }
                        }
                        "right" => {
                            let mut valid_value_found = false;

                            if let Yaml::Integer(right) = value {
                                if *right > 0 {
                                    read_right = Some(*right as u32);
                                    valid_value_found = true;
                                }
                            }

                            if !valid_value_found {
                                warn!("Config contains invalid value for right: \"{:?}\"", value);
                            }
                        }
                        "bottom" => {
                            let mut valid_value_found = false;

                            if let Yaml::Integer(bottom) = value {
                                if *bottom > 0 {
                                    read_bottom = Some(*bottom as u32);
                                    valid_value_found = true;
                                }
                            }

                            if !valid_value_found {
                                warn!("Config contains invalid value for bottom: \"{:?}\"", value);
                            }
                        }
                        "text_prefix" => {
                            if let Yaml::String(text_prefix) = value {
                                read_text_prefix = Some(text_prefix.clone());
                            } else {
                                warn!(
                                    "Config contains invalid value for text prefix \"{:?}\"",
                                    value
                                );
                            }
                        }
                        "text_suffix" => {
                            if let Yaml::String(text_suffix) = value {
                                read_text_suffix = Some(text_suffix.clone());
                            } else {
                                warn!(
                                    "Config contains invalid value for text suffix \"{:?}\"",
                                    value
                                );
                            }
                        }
                        "command" => {
                            if let Yaml::String(command) = value {
                                read_command = Some(command.clone());
                            } else {
                                warn!("Config contains invalid value for command \"{:?}\"", value);
                            }
                        }
                        "is_default" => {
                            if let Yaml::Boolean(is_default) = value {
                                read_is_default = Some(is_default.clone());
                            } else {
                                warn!("Config contains invalid value for default \"{:?}\"", value);
                            }
                        }
                        unknown_key => {
                            warn!("Config contains unknown key {}", unknown_key);
                        }
                    }
                }

                if read_image_filename.is_none() {
                    warn!("Config file is missing an image filename for a meme; skipping");
                    continue;
                }

                if read_font_filename.is_none() {
                    if fonts.is_empty() {
                        warn!("Config file is missing a font for an image; skipping");
                    } else {
                        warn!("Config file is missing a font for an image; using a random font");
                        read_font_filename = Some(fonts.keys().next().unwrap().clone());
                    }
                }

                let image_filename = read_image_filename.clone().unwrap();

                let image = match load_image(&image_filename) {
                    Ok(image) => image,
                    Err(reason) => {
                        warn!("Unable to load image \"{}\": {}", image_filename, reason);
                        continue;
                    }
                };

                let font_name = read_font_filename.unwrap();

                if !fonts.contains_key(&font_name) {
                    match load_font(&font_name) {
                        Ok(font) => {
                            fonts.insert(font_name.clone(), font);
                        }
                        Err(reason) => {
                            warn!("Unable to load font \"{}\": {}", font_name, reason);
                        }
                    }
                }

                // TODO: Find or load the font

                let font_size = read_font_size.unwrap_or(12);
                let scale = Scale {
                    x: font_size as f32,
                    y: font_size as f32,
                };
                let left = read_left.unwrap_or(0);
                let top = read_top.unwrap_or(0);
                let right = read_right.unwrap_or(image.width());
                let bottom = read_bottom.unwrap_or(image.height());
                let center = Point {
                    x: right / 2,
                    y: bottom / 2,
                };
                let text_prefix = read_text_prefix.clone().unwrap_or("".into());
                let text_suffix = read_text_suffix.clone().unwrap_or("".into());
                let command = read_command.clone().unwrap_or("".into());
                let is_default = read_is_default.unwrap_or(false);

                memes.push(Meme {
                    image,
                    font: font_name.clone(),
                    scale,
                    left,
                    top,
                    right,
                    bottom,
                    center,
                    text_prefix,
                    text_suffix,
                    command,
                    is_default,
                });
            } else {
                warn!("Config contains invalid content");
            }
        }
    } else {
        error!("Config file does not appear to contain any meme data or is malformed");
        process::exit(1);
    }

    (fonts, memes)
}

fn get_line_height(font: &Font, scale: Scale) -> u32 {
    let v_metrics = font.v_metrics(scale);

    (v_metrics.line_gap / 2f32 + v_metrics.ascent - v_metrics.descent) as u32
}

fn get_text_width(font: &Font, text: &str, scale: Scale) -> u32 {
    // Return the rightmost edge of the last glyph in the text
    let point = Point { x: 0f32, y: 0f32 };

    let glyph = match font.layout(text, scale, point).last() {
        Some(glyph) => glyph,
        None => return 0,
    };

    match glyph.pixel_bounding_box() {
        Some(point) => point.max.x as u32,
        None => return 0,
    }
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
                    return;
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
                if command.entire.is_empty() {
                    return;
                }

                let text = format!("'{}'?", command.entire.to_uppercase());

                debug!("Creating image for string \"{}\"", text);

                /*
                let data = ctx.data.read();
                let mut base_image = data
                    .get::<BaseImageKey>()
                    .expect("Command create_image: Unable to retrieve base image")
                    .clone();

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

                let (center_x, center_y) = (412, 278);

                let font = data
                    .get::<FontKey>()
                    .expect("Command create_image: Unable to retrieve font");

                let color = Pixel::from_channels(0, 0, 0, 255);
                let scale = Scale { x: 18.0, y: 18.0 };

                let line_height = get_line_height(&font, scale);

                let lines: Vec<&str> = text.lines().collect();
                let mut curr_y = center_y - (line_height * (lines.len() as u32) / 2);

                for line in lines {
                    let x = center_x - get_text_width(&font, &line, scale) / 2;

                    base_image =
                        drawing::draw_text(&mut base_image, color, x, curr_y, scale, &font, &line);

                    curr_y += line_height;
                }

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
                */

                // temp_dir falls out of scope and is automatically deleted
            }
        }
    }
}

fn main() {
    dotenv().ok();
    env_logger::init();

    // Collect basic config
    let discord_bot_token = match env::var("DISCORD_BOT_TOKEN") {
        Ok(token) => token,
        Err(_) => {
            error!("DISCORD_BOT_TOKEN is missing");
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

    let (fonts, memes) = load_memes(&env::var("CONFIG_FILE").unwrap_or("config.yml".into()));

    if memes.is_empty() {
        warn!("No memes were loaded");
    }

    info!("Connecting");

    let mut client = match Client::new(&discord_bot_token, Handler) {
        Ok(client) => client,
        Err(reason) => {
            error!("Unable to create client: {}", reason);
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
        data.insert::<FontsKey>(fonts);
        data.insert::<MemesKey>(memes);
    }

    if let Err(reason) = client.start() {
        error!("Unable to start client: {}", reason);
        process::exit(1);
    }
}
