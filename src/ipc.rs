//! Local bridge between the launcher/runtime (Rust) and the in-game agent.
//!
//! Transport: TCP on 127.0.0.1, newline-delimited JSON. The agent connects, the launcher
//! pushes settings/cosmetics and answers bot observations with commands.
//! Only loopback connections are accepted.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::cosmetics::CosmeticEntry;
use crate::hud_manager::{GameStats, HudLayout, QolSettings};
use crate::trainer::{BotConfig, Command, KbProfile, Obs, TrainerManager};

/// Launcher -> game
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToGame {
    Settings { qol: QolSettings, hud: HudLayout },
    Cosmetics { players: HashMap<String, CosmeticEntry> },
    SpawnBot { bot: BotConfig },
    ClearBots,
    KnockbackTrainer { enabled: bool, profile: KbProfile },
    BotCommand { id: u32, command: Command },
    Bedwars { bridge_trainer: bool },
}

/// Game -> launcher
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FromGame {
    Hello { uuid: String, name: String, version: String },
    Stats { stats: GameStats },
    BotObs { id: u32, obs: Obs },
    /// Players currently in the tab list; the launcher resolves their Frost status.
    TabPlayers { uuids: Vec<String> },
    KnockbackEvent { landed: bool },
}

#[derive(Default)]
pub struct Shared {
    pub stats: GameStats,
    pub connected: bool,
    pub tab_players: Vec<String>,
    pub trainer: TrainerManager,
}

#[derive(Clone)]
pub struct IpcHandle {
    pub shared: Arc<Mutex<Shared>>,
    tx: Arc<Mutex<Option<mpsc::UnboundedSender<ToGame>>>>,
}

impl IpcHandle {
    /// Queue a message for the game. Returns `false` when no game is connected.
    pub fn send(&self, msg: ToGame) -> bool {
        match self.tx.lock().unwrap().as_ref() {
            Some(tx) => tx.send(msg).is_ok(),
            None => false,
        }
    }

    pub fn stats(&self) -> GameStats {
        self.shared.lock().unwrap().stats.clone()
    }

    pub fn connected(&self) -> bool {
        self.shared.lock().unwrap().connected
    }

    /// Register a trainer bot and tell the game to spawn it.
    pub fn spawn_bot(&self, name: &str, style: crate::trainer::CombatStyle, difficulty: f32, legacy: bool) -> bool {
        let bot = self.shared.lock().unwrap().trainer.spawn(name, style, difficulty, legacy);
        self.send(ToGame::SpawnBot { bot })
    }

    pub fn clear_bots(&self) -> bool {
        self.shared.lock().unwrap().trainer.clear();
        self.send(ToGame::ClearBots)
    }
}

/// Bind the loopback listener and serve one game connection at a time.
pub async fn serve(port: u16) -> Result<IpcHandle> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let handle = IpcHandle {
        shared: Arc::new(Mutex::new(Shared::default())),
        tx: Arc::new(Mutex::new(None)),
    };
    let h = handle.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, addr)) = listener.accept().await else { continue };
            if !addr.ip().is_loopback() {
                continue;
            }
            let (tx, rx) = mpsc::unbounded_channel();
            *h.tx.lock().unwrap() = Some(tx);
            h.shared.lock().unwrap().connected = true;
            if let Err(e) = handle_connection(stream, rx, &h).await {
                tracing::warn!("ipc connection ended: {e:#}");
            }
            *h.tx.lock().unwrap() = None;
            h.shared.lock().unwrap().connected = false;
        }
    });
    Ok(handle)
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    mut rx: mpsc::UnboundedReceiver<ToGame>,
    h: &IpcHandle,
) -> Result<()> {
    stream.set_nodelay(true)?;
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line? else { return Ok(()) };
                match serde_json::from_str::<FromGame>(&line) {
                    Ok(msg) => {
                        if let Some(reply) = handle_message(msg, h) {
                            let mut s = serde_json::to_string(&reply)?;
                            s.push('\n');
                            write.write_all(s.as_bytes()).await?;
                        }
                    }
                    Err(e) => tracing::debug!("bad ipc line: {e}"),
                }
            }
            out = rx.recv() => {
                let Some(msg) = out else { return Ok(()) };
                let mut s = serde_json::to_string(&msg)?;
                s.push('\n');
                write.write_all(s.as_bytes()).await?;
            }
        }
    }
}

fn handle_message(msg: FromGame, h: &IpcHandle) -> Option<ToGame> {
    let mut shared = h.shared.lock().unwrap();
    match msg {
        FromGame::Hello { name, version, .. } => {
            tracing::info!("agent connected: {name} on {version}");
            None
        }
        FromGame::Stats { stats } => {
            shared.stats = stats;
            None
        }
        FromGame::TabPlayers { uuids } => {
            shared.tab_players = uuids;
            None
        }
        FromGame::BotObs { id, obs } => shared
            .trainer
            .tick(id, &obs)
            .map(|command| ToGame::BotCommand { id, command }),
        FromGame::KnockbackEvent { landed } => {
            if landed {
                shared.trainer.kb.on_hit_landed();
            } else {
                shared.trainer.kb.on_hit_taken();
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trainer::{CombatStyle, Vec3};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    #[tokio::test]
    async fn bot_roundtrip_over_loopback() {
        // port 0 is not exposed by `serve`, so pick a high port per test run
        let port = 25000 + (std::process::id() % 2000) as u16;
        let handle = serve(port).await.unwrap();

        let stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (r, mut w) = stream.into_split();
        let mut lines = BufReader::new(r).lines();

        // wait until the server registered the connection
        for _ in 0..50 {
            if handle.connected() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(handle.spawn_bot("Bot", CombatStyle::Sword, 0.9, true));
        let spawn: ToGame = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let id = match spawn {
            ToGame::SpawnBot { bot } => bot.id,
            other => panic!("unexpected {other:?}"),
        };

        let obs = Obs {
            bot_pos: Vec3::ZERO,
            bot_yaw: 0.0,
            bot_pitch: 0.0,
            bot_health: 20.0,
            on_ground: true,
            target_pos: Vec3::new(0.0, 0.0, 8.0),
            target_vel: Vec3::ZERO,
            target_blocking: false,
            target_health: 20.0,
            crystal_to_target: None,
            dt: 0.05,
        };
        let msg = serde_json::to_string(&FromGame::BotObs { id, obs }).unwrap() + "\n";
        w.write_all(msg.as_bytes()).await.unwrap();
        let reply: ToGame = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        match reply {
            ToGame::BotCommand { id: rid, command } => {
                assert_eq!(rid, id);
                assert!(command.forward);
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
