#!/usr/bin/env python3
"""
房间信息同步回调服务器
支持拆分后的回调接口和传统接口
"""

import sqlite3
import json
import time
import uuid
from datetime import datetime
from flask import Flask, request, jsonify
from flask_cors import CORS
import threading
import logging

# 配置日志
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = Flask(__name__)
CORS(app)

# 数据库配置
DB_PATH = "callback_data.db"

def init_database():
    """初始化数据库"""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    # 房间同步记录表
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS room_syncs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_id TEXT NOT NULL,
            sync_time INTEGER NOT NULL,
            admin_user_ids TEXT,
            start_time INTEGER,
            current_users INTEGER,
            peak_users INTEGER,
            total_joins INTEGER,
            chat_count INTEGER,
            session_count INTEGER,
            raw_data TEXT,
            event_type TEXT DEFAULT 'legacy'
        )
    ''')
    
    # 聊天记录表
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS chat_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            sync_time INTEGER NOT NULL,
            batch_id TEXT
        )
    ''')
    
    # 会话记录表
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS session_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            join_time INTEGER NOT NULL,
            leave_time INTEGER,
            duration_seconds INTEGER,
            sync_time INTEGER NOT NULL,
            batch_id TEXT
        )
    ''')
    
    # 房间事件表
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS room_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            event_data TEXT NOT NULL,
            timestamp INTEGER NOT NULL
        )
    ''')
    
    # 创建索引
    cursor.execute('CREATE INDEX IF NOT EXISTS idx_room_syncs_room_id ON room_syncs(room_id)')
    cursor.execute('CREATE INDEX IF NOT EXISTS idx_chat_records_room_id ON chat_records(room_id)')
    cursor.execute('CREATE INDEX IF NOT EXISTS idx_session_records_room_id ON session_records(room_id)')
    cursor.execute('CREATE INDEX IF NOT EXISTS idx_room_events_room_id ON room_events(room_id)')
    
    conn.commit()
    conn.close()
    logger.info("数据库初始化完成")

@app.route('/health', methods=['GET'])
def health_check():
    """健康检查"""
    return jsonify({"status": "ok", "timestamp": int(time.time())})

@app.route('/sync/room', methods=['POST'])
def sync_room_legacy():
    """传统房间同步接口（保持向后兼容）"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"error": "No data provided"}), 400
        
        logger.info(f"收到传统房间同步数据: {data.get('room_id')}")
        
        # 存储房间同步记录
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        cursor.execute('''
            INSERT INTO room_syncs (
                room_id, sync_time, admin_user_ids, start_time, 
                current_users, peak_users, total_joins, chat_count, 
                session_count, raw_data, event_type
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ''', (
            data.get('room_id'),
            int(time.time()),
            json.dumps(list(data.get('admin_user_ids', []))),
            data.get('start_time'),
            data.get('stats', {}).get('current_users', 0),
            data.get('stats', {}).get('peak_users', 0),
            data.get('stats', {}).get('total_joins', 0),
            len(data.get('chat_history', [])),
            len(data.get('session_history', [])),
            json.dumps(data),
            'legacy'
        ))
        
        # 存储聊天记录
        for chat in data.get('chat_history', []):
            cursor.execute('''
                INSERT INTO chat_records (room_id, user_id, content, created_at, sync_time)
                VALUES (?, ?, ?, ?, ?)
            ''', (
                data.get('room_id'),
                chat.get('user_id'),
                chat.get('content'),
                chat.get('created_at'),
                int(time.time())
            ))
        
        # 存储会话记录
        for session in data.get('session_history', []):
            cursor.execute('''
                INSERT INTO session_records (room_id, user_id, join_time, leave_time, duration_seconds, sync_time)
                VALUES (?, ?, ?, ?, ?, ?)
            ''', (
                data.get('room_id'),
                session.get('user_id'),
                session.get('join_time'),
                session.get('leave_time'),
                session.get('duration_seconds'),
                int(time.time())
            ))
        
        conn.commit()
        conn.close()
        
        return jsonify({"status": "success", "message": "Room data synced successfully"})
        
    except Exception as e:
        logger.error(f"处理传统房间同步失败: {e}")
        return jsonify({"error": str(e)}), 500

@app.route('/api/room-events', methods=['POST'])
def room_events():
    """房间事件回调接口"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"error": "No data provided"}), 400
        
        event_type = data.get('event_type')
        room_id = data.get('room_id')
        
        logger.info(f"收到房间事件: {event_type} - {room_id}")
        
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        cursor.execute('''
            INSERT INTO room_events (room_id, event_type, event_data, timestamp)
            VALUES (?, ?, ?, ?)
        ''', (
            room_id,
            event_type,
            json.dumps(data),
            data.get('timestamp', int(time.time()))
        ))
        
        conn.commit()
        conn.close()
        
        return jsonify({"status": "success", "message": f"Room event {event_type} recorded"})
        
    except Exception as e:
        logger.error(f"处理房间事件失败: {e}")
        return jsonify({"error": str(e)}), 500

@app.route('/api/chat-history', methods=['POST'])
def chat_history():
    """聊天记录批次回调接口"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"error": "No data provided"}), 400
        
        room_id = data.get('room_id')
        batch_id = data.get('batch_id')
        messages = data.get('messages', [])
        is_last_batch = data.get('is_last_batch', False)
        
        logger.info(f"收到聊天记录批次: {room_id} - {batch_id} - {len(messages)}条消息")
        
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        # 存储聊天记录
        for message in messages:
            cursor.execute('''
                INSERT INTO chat_records (room_id, user_id, content, created_at, sync_time, batch_id)
                VALUES (?, ?, ?, ?, ?, ?)
            ''', (
                room_id,
                message.get('user_id'),
                message.get('content'),
                message.get('created_at'),
                data.get('timestamp', int(time.time())),
                batch_id
            ))
        
        conn.commit()
        conn.close()
        
        return jsonify({
            "status": "success", 
            "message": f"Chat history batch {batch_id} processed",
            "is_last_batch": is_last_batch
        })
        
    except Exception as e:
        logger.error(f"处理聊天记录批次失败: {e}")
        return jsonify({"error": str(e)}), 500

@app.route('/api/session-history', methods=['POST'])
def session_history():
    """会话历史批次回调接口"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"error": "No data provided"}), 400
        
        room_id = data.get('room_id')
        batch_id = data.get('batch_id')
        sessions = data.get('sessions', [])
        is_last_batch = data.get('is_last_batch', False)
        
        logger.info(f"收到会话历史批次: {room_id} - {batch_id} - {len(sessions)}条记录")
        
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        # 存储会话记录
        for session in sessions:
            cursor.execute('''
                INSERT INTO session_records (room_id, user_id, join_time, leave_time, duration_seconds, sync_time, batch_id)
                VALUES (?, ?, ?, ?, ?, ?, ?)
            ''', (
                room_id,
                session.get('user_id'),
                session.get('join_time'),
                session.get('leave_time'),
                session.get('duration_seconds'),
                data.get('timestamp', int(time.time())),
                batch_id
            ))
        
        conn.commit()
        conn.close()
        
        return jsonify({
            "status": "success", 
            "message": f"Session history batch {batch_id} processed",
            "is_last_batch": is_last_batch
        })
        
    except Exception as e:
        logger.error(f"处理会话历史批次失败: {e}")
        return jsonify({"error": str(e)}), 500

@app.route('/api/periodic-sync', methods=['POST'])
def periodic_sync():
    """定时同步回调接口"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"error": "No data provided"}), 400
        
        room_id = data.get('room_id')
        room_info = data.get('room_info', {})
        last_sync_time = data.get('last_sync_time')
        
        logger.info(f"收到定时同步: {room_id} - {last_sync_time}")
        
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        cursor.execute('''
            INSERT INTO room_syncs (
                room_id, sync_time, admin_user_ids, start_time, 
                current_users, peak_users, total_joins, chat_count, 
                session_count, raw_data, event_type
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ''', (
            room_id,
            last_sync_time,
            json.dumps(room_info.get('admin_user_ids', [])),
            room_info.get('created_at'),
            room_info.get('current_connections', 0),
            0, 0, 0, 0, 0,  # 这些字段在定时同步中可能没有
            json.dumps(data),
            'periodic'
        ))
        
        conn.commit()
        conn.close()
        
        return jsonify({"status": "success", "message": "Periodic sync recorded"})
        
    except Exception as e:
        logger.error(f"处理定时同步失败: {e}")
        return jsonify({"error": str(e)}), 500

@app.route('/rooms', methods=['GET'])
def get_rooms():
    """获取房间列表"""
    try:
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        cursor.execute('''
            SELECT DISTINCT room_id, MAX(sync_time) as last_sync
            FROM room_syncs 
            GROUP BY room_id 
            ORDER BY last_sync DESC
        ''')
        
        rooms = []
        for row in cursor.fetchall():
            room_id, last_sync = row
            rooms.append({
                "room_id": room_id,
                "last_sync": last_sync,
                "last_sync_formatted": datetime.fromtimestamp(last_sync).strftime('%Y-%m-%d %H:%M:%S')
            })
        
        conn.close()
        return jsonify({"rooms": rooms})
        
    except Exception as e:
        logger.error(f"获取房间列表失败: {e}")
        return jsonify({"error": str(e)}), 500

@app.route('/rooms/<room_id>', methods=['GET'])
def get_room_details(room_id):
    """获取房间详情"""
    try:
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        # 获取最新的房间同步记录
        cursor.execute('''
            SELECT * FROM room_syncs 
            WHERE room_id = ? 
            ORDER BY sync_time DESC 
            LIMIT 1
        ''', (room_id,))
        
        room_sync = cursor.fetchone()
        if not room_sync:
            return jsonify({"error": "Room not found"}), 404
        
        # 获取聊天记录数量
        cursor.execute('SELECT COUNT(*) FROM chat_records WHERE room_id = ?', (room_id,))
        chat_count = cursor.fetchone()[0]
        
        # 获取会话记录数量
        cursor.execute('SELECT COUNT(*) FROM session_records WHERE room_id = ?', (room_id,))
        session_count = cursor.fetchone()[0]
        
        # 获取最近的事件
        cursor.execute('''
            SELECT event_type, event_data, timestamp 
            FROM room_events 
            WHERE room_id = ? 
            ORDER BY timestamp DESC 
            LIMIT 10
        ''', (room_id,))
        
        recent_events = []
        for row in cursor.fetchall():
            event_type, event_data, timestamp = row
            recent_events.append({
                "event_type": event_type,
                "event_data": json.loads(event_data),
                "timestamp": timestamp,
                "formatted_time": datetime.fromtimestamp(timestamp).strftime('%Y-%m-%d %H:%M:%S')
            })
        
        conn.close()
        
        return jsonify({
            "room_id": room_id,
            "last_sync": room_sync[2],
            "chat_count": chat_count,
            "session_count": session_count,
            "recent_events": recent_events
        })
        
    except Exception as e:
        logger.error(f"获取房间详情失败: {e}")
        return jsonify({"error": str(e)}), 500

@app.route('/stats', methods=['GET'])
def get_stats():
    """获取统计信息"""
    try:
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        # 总房间数
        cursor.execute('SELECT COUNT(DISTINCT room_id) FROM room_syncs')
        total_rooms = cursor.fetchone()[0]
        
        # 总聊天记录数
        cursor.execute('SELECT COUNT(*) FROM chat_records')
        total_chat_records = cursor.fetchone()[0]
        
        # 总会话记录数
        cursor.execute('SELECT COUNT(*) FROM session_records')
        total_session_records = cursor.fetchone()[0]
        
        # 总事件数
        cursor.execute('SELECT COUNT(*) FROM room_events')
        total_events = cursor.fetchone()[0]
        
        # 今日同步数
        today_start = int(time.time()) - (24 * 60 * 60)
        cursor.execute('SELECT COUNT(*) FROM room_syncs WHERE sync_time >= ?', (today_start,))
        today_syncs = cursor.fetchone()[0]
        
        conn.close()
        
        return jsonify({
            "total_rooms": total_rooms,
            "total_chat_records": total_chat_records,
            "total_session_records": total_session_records,
            "total_events": total_events,
            "today_syncs": today_syncs,
            "timestamp": int(time.time())
        })
        
    except Exception as e:
        logger.error(f"获取统计信息失败: {e}")
        return jsonify({"error": str(e)}), 500

if __name__ == '__main__':
    # 初始化数据库
    init_database()
    
    # 启动服务器
    logger.info("启动回调服务器...")
    app.run(host='0.0.0.0', port=8080, debug=True) 