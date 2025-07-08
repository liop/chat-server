
// ====================================================================================
// src/handler.rs - 核心业务逻辑处理器
// ====================================================================================
use crate::{
    db,
    models::{ControlMessage, DbWriteCommand, InternalMessage, RoomDetailsResponse, RoomState, RoomStats, StatsQuery, WsMessage},
    state::AppState,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use uuid::Uuid;
use axum::extract::ws::WebSocket;

struct ConnectionInfo {
    sender: mpsc::Sender<WsMessage>,
    join_time: Instant,
    is_admin: bool,
    nickname: String,
}

// RAII Guard for connection counting
struct ConnectionGuard {
    count: Arc<AtomicU32>,
}

impl ConnectionGuard {
    fn new(count: Arc<AtomicU32>) -> Self {
        count.fetch_add(1, Ordering::Relaxed);
        Self { count }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::Relaxed);
    }
}

// 启动一个房间的中央处理器
pub async fn start_room_handler(room_id: Uuid, state: Arc<AppState>) {
    let (high_prio_tx, high_prio_rx) = mpsc::channel(100);
    let (normal_prio_tx, normal_prio_rx) = mpsc::channel(100);
    let (db_writer_tx, db_writer_rx) = mpsc::channel(1024);
    let (control_tx, control_rx) = mpsc::channel(32);
    let (stats_tx, stats_rx) = mpsc::channel(32);

    db::spawn_db_writer(state.db_pool.clone(), db_writer_rx);

    let room_state = RoomState {
        high_prio_tx,
        normal_prio_tx,
        db_writer_tx: db_writer_tx.clone(),
        control_tx,
        stats_tx,
    };

    state.rooms.lock().await.insert(room_id, room_state);

    tokio::spawn(room_message_loop(
        room_id,
        state,
        high_prio_rx,
        normal_prio_rx,
        control_rx,
        stats_rx,
    ));
}

// 处理单个WebSocket连接
pub async fn handle_socket(socket: WebSocket, state: Arc<AppState>, room_id: Uuid, user_id: String, nickname: String) {
    let _conn_guard = ConnectionGuard::new(state.total_connections.clone());
    let conn_id = Uuid::new_v4();
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let room_tx = {
        let rooms = state.rooms.lock().await;
        if let Some(room) = rooms.get(&room_id) {
            Some(room.normal_prio_tx.clone())
        } else {
            let error_msg = serde_json::to_string(&WsMessage::Error { message: "房间已关闭".to_string() }).unwrap();
            let _ = ws_sender.send(axum::extract::ws::Message::Text(error_msg)).await;
            return;
        }
    };
    let Some(room_tx) = room_tx else { return; };

    let (tx, mut rx) = mpsc::channel(10);

    if room_tx.send(InternalMessage {
        conn_id, user_id: user_id.clone(), nickname: nickname.clone(), room_id,
        content: WsMessage::UserJoined { user_id: user_id.clone(), nickname: nickname.clone() },
        sender: Some(tx.clone()),
    }).await.is_err() {
        tracing::warn!("Failed to register new user, room handler might be down.");
        return;
    }

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let text_msg = serde_json::to_string(&msg).unwrap();
            if ws_sender.send(axum::extract::ws::Message::Text(text_msg)).await.is_err() { break; }
        }
    });

    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Ok(ws_msg) = WsMessage::try_from(msg) {
            if room_tx.send(InternalMessage {
                conn_id, user_id: user_id.clone(), nickname: nickname.clone(), room_id,
                content: ws_msg, sender: None,
            }).await.is_err() { break; }
        }
    }

    let _ = room_tx.send(InternalMessage {
        conn_id, user_id: user_id.clone(), nickname: nickname.clone(), room_id,
        content: WsMessage::UserLeft { user_id: user_id.clone(), nickname: nickname.clone() }, sender: None,
    }).await;
}

// 房间的中央消息处理循环
pub async fn room_message_loop(
    room_id: Uuid,
    state: Arc<AppState>,
    mut high_prio_rx: mpsc::Receiver<InternalMessage>,
    mut normal_prio_rx: mpsc::Receiver<InternalMessage>,
    mut control_rx: mpsc::Receiver<ControlMessage>,
    mut stats_rx: mpsc::Receiver<StatsQuery>,
) {
    let mut connections: HashMap<Uuid, ConnectionInfo> = HashMap::new();
    let mut user_id_to_conn_id: HashMap<String, Uuid> = HashMap::new();
    let mut muted_users: HashSet<String> = HashSet::new();
    let mut admin_users = db::load_admins_for_room(&state.db_pool, room_id).await.unwrap_or_default();
    let mut banned_users = db::load_bans_for_room(&state.db_pool, room_id).await.unwrap_or_default();
    let mut stats = RoomStats::default();
    let start_time = chrono::Utc::now().timestamp();

    loop {
        tokio::select! {
            Some(msg) = high_prio_rx.recv() => {
                handle_message(msg, &mut connections, &mut user_id_to_conn_id, &mut muted_users, &mut banned_users, &admin_users, &state, &mut stats).await;
            },
            Some(msg) = normal_prio_rx.recv() => {
                handle_message(msg, &mut connections, &mut user_id_to_conn_id, &mut muted_users, &mut banned_users, &admin_users, &state, &mut stats).await;
            },
            Some(ctrl_msg) = control_rx.recv() => {
                match ctrl_msg {
                    ControlMessage::ResetAdmins(new_admins) => {
                        admin_users = new_admins;
                        tracing::info!("Admins for room {} have been reset.", room_id);
                    }
                    ControlMessage::UnbanUser(user_id) => {
                        if banned_users.remove(&user_id) {
                            tracing::info!("User {} has been unbanned from room {}.", user_id, room_id);
                        }
                    }
                }
            },
            Some(query) = stats_rx.recv() => {
                let response = RoomDetailsResponse {
                    room_id,
                    admin_user_ids: admin_users.clone(),
                    start_time,
                    stats: stats.clone(),
                };
                let _ = query.response_tx.send(response);
            },
            else => {
                tracing::info!("Room {} handler shutting down.", room_id);
                // 广播当前房间人数
                broadcast(&connections, WsMessage::RoomStats { current_users: stats.current_users, peak_users: stats.peak_users }, None).await;
                break;
            }
        }
    }
}

// 具体的消息处理逻辑
async fn handle_message(
    msg: InternalMessage,
    connections: &mut HashMap<Uuid, ConnectionInfo>,
    user_id_to_conn_id: &mut HashMap<String, Uuid>,
    muted_users: &mut HashSet<String>,
    banned_users: &mut HashSet<String>,
    admin_users: &HashSet<String>,
    state: &Arc<AppState>,
    stats: &mut RoomStats,
) {
    let db_writer_tx = {
        let rooms = state.rooms.lock().await;
        rooms.get(&msg.room_id).map(|r| r.db_writer_tx.clone())
    };
    let Some(db_writer_tx) = db_writer_tx else { return; };

    match msg.content {
        WsMessage::UserJoined { ref user_id, ref nickname } => {
            let sender = msg.sender.expect("UserJoined message must have a sender");

            if banned_users.contains(&msg.user_id) {
                let _ = sender.send(WsMessage::Error { message: "你已被踢出该房间，无法再次加入".to_string() }).await;
                return;
            }

            if let Some(old_conn_id) = user_id_to_conn_id.remove(&msg.user_id) {
                if let Some(old_conn) = connections.remove(&old_conn_id) {
                    stats.current_users = stats.current_users.saturating_sub(1);
                    let _ = old_conn.sender.send(WsMessage::YouAreKicked).await;
                }
            }
            
            let is_admin = admin_users.contains(&msg.user_id);
            let is_muted = muted_users.contains(&msg.user_id);
            let welcome_msg = WsMessage::WelcomeInfo { user_id: msg.user_id.clone(), nickname: msg.nickname.clone(), is_muted };
            if sender.send(welcome_msg).await.is_err() { return; }

            let conn_info = ConnectionInfo { sender, join_time: Instant::now(), is_admin, nickname: msg.nickname.clone() };
            connections.insert(msg.conn_id, conn_info);
            user_id_to_conn_id.insert(msg.user_id.clone(), msg.conn_id);

            stats.current_users += 1;
            stats.total_joins += 1;
            if stats.current_users > stats.peak_users {
                stats.peak_users = stats.current_users;
            }

            let _ = db_writer_tx.send(DbWriteCommand::UserJoined { user_id: msg.user_id.clone(), nickname: msg.nickname.clone(), room_id: msg.room_id }).await;
        }
        WsMessage::UserLeft { ref user_id, ref nickname } => {
            if let Some(conn_info) = connections.remove(&msg.conn_id) {
                user_id_to_conn_id.remove(&msg.user_id);
                stats.current_users = stats.current_users.saturating_sub(1);
                let _ = db_writer_tx.send(DbWriteCommand::UserLeft { user_id: msg.user_id.clone(), nickname: msg.nickname.clone(), room_id: msg.room_id, join_time: conn_info.join_time }).await;
            }
        }
        WsMessage::SendMessage { ref content } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) { info } else { return; };
            if muted_users.contains(&msg.user_id) {
                let _ = conn_info.sender.send(WsMessage::YouAreMuted).await;
                return;
            }
            let _ = db_writer_tx.send(DbWriteCommand::ChatMessage { user_id: msg.user_id.clone(), nickname: msg.nickname.clone(), room_id: msg.room_id, content: content.clone() }).await;
            broadcast(connections, WsMessage::Message { from: msg.user_id, nickname: msg.nickname, content: content.clone(), is_admin: conn_info.is_admin }, None).await;
        }
        WsMessage::KickUser { user_id } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) { info } else { return; };
            if !conn_info.is_admin { return; }
            
            banned_users.insert(user_id.clone());
            let _ = db_writer_tx.send(DbWriteCommand::BanUser { user_id: user_id.clone(), room_id: msg.room_id }).await;

            if let Some(target_conn_id) = user_id_to_conn_id.remove(&user_id) {
                if let Some(target_conn) = connections.remove(&target_conn_id) {
                    stats.current_users = stats.current_users.saturating_sub(1);
                    let _ = target_conn.sender.send(WsMessage::YouAreKicked).await;
                }
            }
        }
        WsMessage::MuteUser { user_id } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) { info } else { return; };
            if !conn_info.is_admin { return; }
            muted_users.insert(user_id.clone());
        }
        _ => {}
    }
}

// 广播消息给房间内的所有用户
async fn broadcast(connections: &HashMap<Uuid, ConnectionInfo>, msg: WsMessage, exclude_conn_id: Option<Uuid>) {
    for (conn_id, conn_info) in connections.iter() {
        if Some(*conn_id) == exclude_conn_id { continue; }
        if conn_info.sender.send(msg.clone()).await.is_err() {}
    }
}



impl TryFrom<axum::extract::ws::Message> for WsMessage {
    type Error = serde_json::Error;
    fn try_from(msg: axum::extract::ws::Message) -> Result<Self, <WsMessage as TryFrom<axum::extract::ws::Message>>::Error> {
        if let axum::extract::ws::Message::Text(text) = msg {
            serde_json::from_str(&text)
        } else {
            Err(serde_json::from_str::<WsMessage>("").unwrap_err())
        }
    }
}
