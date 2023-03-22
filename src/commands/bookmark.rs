use crate::command::{Command, CommandInput};
use crate::error::InteractionError;

use async_trait::async_trait;
use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE};
use reqwest::StatusCode;
use twilight_model::application::command::CommandType;
use twilight_model::channel::message::component::{ActionRow, Button, ButtonStyle};
use crate::input::SharedInput;
use twilight_model::channel::message::{Component, MessageFlags, ReactionType, Embed};
use twilight_model::guild::{Emoji, Guild};
use twilight_model::http::interaction::InteractionResponseData;
use twilight_model::id::Id;
use twilight_model::id::marker::{GenericMarker, MessageMarker};
use twilight_util::builder::InteractionResponseDataBuilder;
use twilight_util::builder::embed::{EmbedBuilder, EmbedAuthorBuilder, ImageSource, EmbedFooterBuilder};
use worker::console_log;
use regex::Regex;

fn replace_links_with_markdown(text: &str) -> String {
    let mdlink_regex = Regex::new(r#"\[.*?\]\(.*?\)"#).unwrap();
    let mut replaced_text = text.to_string();

    // Match and replace existing markdown links
    for mdlink in mdlink_regex.find_iter(text) {
        let url = &text[mdlink.start() + 1..mdlink.end() - 1];
        let markdown_link = format!("[{}]({})", url, url);
        replaced_text = replaced_text.replace(&text[mdlink.start()..mdlink.end()], &markdown_link);
    }

    // Match and replace non-markdown links
    let link_regex = Regex::new(r#"(?P<url>https?://[^\s]+)"#).unwrap();
    replaced_text = link_regex.replace_all(&replaced_text, "[${url}](${url})").to_string();

    replaced_text
}

pub(crate) struct Bookmark {}

#[async_trait(?Send)]
impl Command for Bookmark {
    async fn respond(
        &self,
        input: &CommandInput,
    ) -> Result<InteractionResponseData, InteractionError> {
        if input.guild_id.is_none() {
            return Ok(InteractionResponseDataBuilder::new()
                .content("This command can only be used in a server")
                .flags(MessageFlags::EPHEMERAL)
                .build());
        }

        console_log!("Starting...");
        
        let client = input.http_client()?;
        // Create a new dm channel
        let url = format!("https://discord.com/api/v10/users/@me/channels");

        let serialized = serde_json::to_string(&serde_json::json!({
            "recipient_id": input.uid()?.to_string()
        }))?;
        console_log!("serialized: {:?}", serialized);
        if let Ok(response) = client.post(url).body(serialized).send().await {
            let status = response.status();
            let text: String = response.text().await?;
            console_log!("[CREATE DM CHANNEL] response: {:?}", text);
            match status {
                StatusCode::OK => {
                    let channel_id: String =
                        match serde_json::from_str::<serde_json::Value>(&text)?
                            .get("id")
                        {
                            Some(id) => id.as_str().unwrap().to_string(),
                            None => {
                                return Ok(InteractionResponseDataBuilder::new()
                                    .content(
                                        "An error occured while creating a dm channel with you",
                                    )
                                    .flags(MessageFlags::EPHEMERAL)
                                    .build())
                            }
                        };

                    let url = format!(
                        "https://discord.com/api/v10/channels/{}/messages",
                        channel_id
                    );

                    
                    
                    let og_msg_id = Id::<MessageMarker>::new(input.target_id.unwrap().get());
                    let msg_data = input.resolved.as_ref().unwrap().messages.get(&og_msg_id).unwrap();
                    let t_url = format!(
                        "https://discord.com/channels/{}/{}/{}",
                        input.guild_id.unwrap(),
                        input.channel_id.unwrap(),
                        og_msg_id
                    );
                    let components = input.default_components(&t_url);
                    let mut embeds = msg_data.embeds.clone().into_iter().filter(|e| e.kind == "rich").collect::<Vec<Embed>>();

                    let guild_text = client.get(format!("https://discord.com/api/v10/guilds/{}", input.guild_id.unwrap())).send().await?.text().await?;
                    let guild: Guild = serde_json::from_str(&guild_text)?;
                    let author = EmbedAuthorBuilder::new(msg_data.author.name.clone())
                            .icon_url(ImageSource::url(format!("https://cdn.discordapp.com/avatars/{}/{}.png", msg_data.author.id, msg_data.author.avatar.unwrap())).unwrap())
                            .build();

                    let footer = EmbedFooterBuilder::new(format!("{}", guild.name))
                            .icon_url(ImageSource::url(format!("https://cdn.discordapp.com/icons/{}/{}.png", guild.id, guild.icon.unwrap())).unwrap())
                            .build();
                        
                    if msg_data.content.len() > 0 {
                        embeds.insert(0,
                            EmbedBuilder::new()
                                .description(msg_data.content.clone())
                                .build());
                        
                    };
                    
                    // Overiding the footer(server) and author data
                    embeds[0].author = Some(author);
                    embeds[0].footer = Some(footer);
                    
                    for embed in embeds.iter_mut() {
                        embed.description = Some(replace_links_with_markdown(&embed.description.clone().unwrap()));
                    }
                    // Attachments text
                    let attachments = msg_data.attachments.clone();
                    
                    if attachments.len() > 0 {
                        let fmt = attachments.iter().map(|a| format!("[{}]({})", a.filename,a.url)).collect::<Vec<String>>().join("\n> ");
                        embeds[0].description = Some(format!("{}\n**Attachments:**\n> {}", embeds[0].description.clone().unwrap(), fmt));
                    }


                    let serialized = serde_json::to_string(&serde_json::json!({
                        "embeds": embeds,
                        "components": components
                    }))?;


                    if let Ok(response) = client.post(url).body(serialized).send().await {
                        let status = response.status();
                        console_log!("text: {:?}", &response.text().await);
                        match status {
                            StatusCode::OK => {
                                return Ok(InteractionResponseDataBuilder::new()
                                    .components(vec![Component::ActionRow(ActionRow {
                                        components: vec![Component::Button(Button {
                                            custom_id: Some("bookmark".to_string()),
                                            disabled: true,
                                            emoji: Some(
                                                ReactionType::Unicode {
                                                    name: "🔖".to_string(),
                                                },
                                            ),
                                            label: Some("Bookmarked".to_string()),
                                            style: ButtonStyle::Primary,
                                            url: None,
                                        })],
                                    })])
                                    .flags(MessageFlags::EPHEMERAL)
                                    .build())
                            }

                            StatusCode::FORBIDDEN => {
                                return Ok(InteractionResponseDataBuilder::new()
                                    .content("Open your dms in this server to use this command")
                                    .flags(MessageFlags::EPHEMERAL)
                                    .build())
                            }

                            _ => {
                                return Ok(InteractionResponseDataBuilder::new()
                                    .content("An error occured while sending a message in this channel")
                                    .flags(MessageFlags::EPHEMERAL)
                                    .build())
                            }
                        }
                    }
                }

                StatusCode::FORBIDDEN => {
                    return Ok(InteractionResponseDataBuilder::new()
                        .content("The bot is not authorized to create a dm channel with you")
                        .flags(MessageFlags::EPHEMERAL)
                        .build())
                }

                err => {
                    return Ok(InteractionResponseDataBuilder::new()
                        .content(format!("An error occured while creating a dm channel with you ({:?})", err))
                        .flags(MessageFlags::EPHEMERAL)
                        .build())
                }
            }
        }

        Ok(InteractionResponseDataBuilder::new()
            .content("Open your dms to the bot")
            .flags(MessageFlags::EPHEMERAL)
            .build())
    }

    fn name(&self) -> String {
        "bookmark".into()
    }

    fn kind(&self) -> CommandType {
        CommandType::Message
    }
}