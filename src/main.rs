#![allow(non_snake_case)]

use std::env;

use serenity::async_trait;
use serenity::builder::{CreateButton, CreateInteractionResponseData};
use serenity::model::gateway::Ready;
use serenity::model::id::RoleId;
use serenity::model::interactions::{Interaction, InteractionResponseType};
use serenity::model::interactions::application_command::{ApplicationCommand, ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType};
use serenity::model::interactions::message_component::ButtonStyle;
use serenity::model::prelude::application_command::ApplicationCommandInteractionDataOption;
use serenity::model::prelude::InteractionType;
use serenity::model::prelude::message_component::{ComponentType, MessageComponentInteraction};
use serenity::prelude::*;

const MAX_BUTTONS_PER_ACTION_ROW: usize = 5;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        if let Err(err) = ApplicationCommand::create_global_application_command(&ctx.http, |command| {
            command.name("create").description("Create a new role-giver button");
            command.create_option(|option| {
                option.name("message").description("The message that will be displayed above buttons")
                .kind(ApplicationCommandOptionType::String).required(true)
            });
            command.create_option(|option| {
                option.name("role1").description("Role").kind(ApplicationCommandOptionType::Role).required(true)
            });
            for i in 2..=10 {
                command.create_option(|option| {
                    option.name(format!("role{}", i)).description("Role").kind(ApplicationCommandOptionType::Role)
                });
            }
            command
        }).await {
            eprintln!("Error sending commands: {err}");
        } else {
            println!("Commands has been send successfully.");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::ApplicationCommand(command) => {

                // Help functions
                async fn response<F>(ctx: &Context, command: &ApplicationCommandInteraction, f: F)
                    where for<'b, 'c> F: FnOnce(&'b mut CreateInteractionResponseData<'c>) -> &'b mut CreateInteractionResponseData<'c>
                {
                    if let Err(err) = command.create_interaction_response(&ctx.http, |response| {
                        response.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(f)
                    }).await {
                        eprintln!("Couldn't send command response: {}", err);
                    }
                }

                async fn error(msg: impl ToString, ctx: &Context, command: &ApplicationCommandInteraction) {
                    response(ctx, command, |message| message.content(format!("Error: {}", msg.to_string()))).await
                }

                match command.data.name.as_str() {
                    "create" => {
                        // Validate inputs
                        let options = &command.data.options;

                        if command.data.options.len() == 0 {
                            error("No role has been provided", &ctx, &command).await;
                            return;
                        }
                        if command.data.options.len() > 10 {
                            error("Too many roles", &ctx, &command).await;
                            return;
                        }

                        let mut iter = options.iter();

                        let resp_message = match iter.next() {
                            Some(ApplicationCommandInteractionDataOption { resolved: Some(ApplicationCommandInteractionDataOptionValue::String(message)), .. }) => {
                                // The first parameter must be the message
                                message
                            },
                            Some(_) => {
                                error("Message is not a string", &ctx, &command).await;
                                return;
                            },
                            None => {
                                error("Message parameter missing", &ctx, &command).await;
                                return;
                            },
                        };

                        let mut roles = Vec::with_capacity(10);
                        for option in iter {
                            if let Some(ApplicationCommandInteractionDataOptionValue::Role(role)) = &option.resolved {
                                roles.push(role);
                            } else {
                                error(format!("{} is not a role", option.name), &ctx, &command).await;
                                return;
                            }
                        }

                        let mut roles = roles.iter().peekable();

                        // Send buttons
                        response(&ctx, &command, |message| {
                            message.content(resp_message)
                            .components(|comp| {
                                while let Some(_) = roles.peek() {
                                    comp.create_action_row(|action_row| {
                                        for _ in 0..MAX_BUTTONS_PER_ACTION_ROW {
                                            match roles.next() {
                                                Some(role) => {
                                                    let mut button = CreateButton::default();
                                                    button.style(ButtonStyle::Success);
                                                    button.custom_id(role.id);
                                                    button.label(role.name.clone());
                                                    action_row.add_button(button);
                                                },
                                                None => break,
                                            };
                                        }
                                        action_row
                                    });
                                }
                                comp
                            })
                        }).await;
                    },
                    _ => { error("Invalid command", &ctx, &command).await; },
                };
            }
            Interaction::MessageComponent(message) => {
                if message.kind != InteractionType::MessageComponent || message.data.component_type != ComponentType::Button {
                    response("Error: Invalid", &ctx, &message).await;
                    return;
                }

                async fn response(msg: &str, ctx: &Context, message: &MessageComponentInteraction) {
                    if let Err(err) = message.create_interaction_response(&ctx.http, |response| {
                        response.kind(InteractionResponseType::ChannelMessageWithSource).interaction_response_data(|message| message.ephemeral(true).content(msg))
                    }).await {
                        eprintln!("Couldn't send command response: {}", err);
                    }

                }

                let role_id: u64 = match message.data.custom_id.parse() {
                    Ok(role_id) => role_id,
                    Err(_) => {
                        response("Error: Invalid role", &ctx, &message).await;
                        return;
                    },
                };

                if let Some(member) = &message.member {
                    if member.roles.contains(&RoleId(role_id)) {
                        match ctx.http.remove_member_role(member.guild_id.0, member.user.id.0, role_id, Some("Role Buttons")).await {
                            Ok(_) => {
                                response("Successfully removed role!", &ctx, &message).await;
                            },
                            Err(_) => {
                                response("Error: Couldn't remove role!", &ctx, &message).await;
                            },
                        }
                    } else {
                        match ctx.http.add_member_role(member.guild_id.0, member.user.id.0, role_id, Some("Role Buttons")).await {
                            Ok(_) => {
                                response("Successfully added role!", &ctx, &message).await;
                            },
                            Err(_) => {
                                response("Error: Couldn't add role!", &ctx, &message).await;
                            },
                        }
                    }
                } else {
                    response("Error: Invalid", &ctx, &message).await;
                }
            },
            _ => {},
        }
    }
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // Build our client.
    let mut client = Client::builder(token, GatewayIntents::empty())
    .event_handler(Handler)
    .await
    .expect("Error creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
