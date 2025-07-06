#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
房间信息同步回调服务器示例
用于接收Rust WebSocket服务器的房间数据同步
"""

from flask import Flask, request, jsonify
import json
import logging
from datetime import datetime
import sqlite3
import os
from typing import Dict, List, Any

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

app = Flask(__name__)

# 数据库文件路径
DB_PATH = "callback_data.db"

def init_database():
    """初始化数据库表"""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    # 创建房间同步记录表
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS room_syncs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_id TEXT NOT NULL,
            sync_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            admin_user_ids TEXT,
            start_time INTEGER,
            current_users INTEGER,
            peak_users INTEGER,
            total_joins INTEGER,
            chat_count INTEGER,
            session_count INTEGER,
            raw_data TEXT
        )
    ''')
    
    # 创建聊天记录表
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS chat_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER,
            sync_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    ''')
    
    # 创建会话记录表
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS session_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            join_time INTEGER,
            leave_time INTEGER,
            duration_seconds INTEGER,
            sync_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    ''')
    
    conn.commit()
    conn.close()
    logger.info("数据库初始化完成")

@app.route('/health', methods=['GET'])
def health_check():
    """健康检查端点"""
    return jsonify({"status": "ok", "timestamp": datetime.now().isoformat()})

@app.route('/sync/room', methods=['POST'])
def sync_room_data():
    """接收房间数据同步"""
    try:
        # 获取JSON数据
        data = request.get_json()
        if not data:
            return jsonify({"error": "无效的JSON数据"}), 400
        
        logger.info(f"收到房间同步数据: {data.get('room_id', 'unknown')}")
        
        # 解析数据
        room_id = data.get('room_id')
        admin_user_ids = data.get('admin_user_ids', [])
        start_time = data.get('start_time', 0)
        stats = data.get('stats', {})
        chat_history = data.get('chat_history', [])
        session_history = data.get('session_history', [])
        
        # 保存到数据库
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        # 保存房间同步记录
        cursor.execute('''
            INSERT INTO room_syncs (
                room_id, admin_user_ids, start_time, 
                current_users, peak_users, total_joins,
                chat_count, session_count, raw_data
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ''', (
            room_id,
            json.dumps(admin_user_ids),
            start_time,
            stats.get('current_users', 0),
            stats.get('peak_users', 0),
            stats.get('total_joins', 0),
            len(chat_history),
            len(session_history),
            json.dumps(data)
        ))
        
        # 保存聊天记录
        for chat in chat_history:
            cursor.execute('''
                INSERT INTO chat_records (room_id, user_id, content, created_at)
                VALUES (?, ?, ?, ?)
            ''', (
                room_id,
                chat.get('user_id'),
                chat.get('content'),
                chat.get('created_at')
            ))
        
        # 保存会话记录
        for session in session_history:
            cursor.execute('''
                INSERT INTO session_records (room_id, user_id, join_time, leave_time, duration_seconds)
                VALUES (?, ?, ?, ?, ?)
            ''', (
                room_id,
                session.get('user_id'),
                session.get('join_time'),
                session.get('leave_time'),
                session.get('duration_seconds')
            ))
        
        conn.commit()
        conn.close()
        
        logger.info(f"房间 {room_id} 数据同步成功")
        return jsonify({
            "status": "success",
            "message": f"房间 {room_id} 数据同步成功",
            "timestamp": datetime.now().isoformat()
        })
        
    except Exception as e:
        logger.error(f"处理房间同步数据时出错: {str(e)}")
        return jsonify({"error": f"处理失败: {str(e)}"}), 500

@app.route('/rooms', methods=['GET'])
def list_rooms():
    """列出所有同步的房间"""
    try:
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        cursor.execute('''
            SELECT room_id, sync_time, current_users, peak_users, total_joins, chat_count, session_count
            FROM room_syncs
            ORDER BY sync_time DESC
        ''')
        
        rooms = []
        for row in cursor.fetchall():
            rooms.append({
                "room_id": row[0],
                "sync_time": row[1],
                "current_users": row[2],
                "peak_users": row[3],
                "total_joins": row[4],
                "chat_count": row[5],
                "session_count": row[6]
            })
        
        conn.close()
        return jsonify({"rooms": rooms})
        
    except Exception as e:
        logger.error(f"获取房间列表时出错: {str(e)}")
        return jsonify({"error": f"获取失败: {str(e)}"}), 500

@app.route('/rooms/<room_id>', methods=['GET'])
def get_room_details(room_id):
    """获取特定房间的详细信息"""
    try:
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        # 获取房间基本信息
        cursor.execute('''
            SELECT sync_time, admin_user_ids, start_time, current_users, peak_users, total_joins, raw_data
            FROM room_syncs
            WHERE room_id = ?
            ORDER BY sync_time DESC
            LIMIT 1
        ''', (room_id,))
        
        room_data = cursor.fetchone()
        if not room_data:
            return jsonify({"error": "房间不存在"}), 404
        
        # 获取聊天记录
        cursor.execute('''
            SELECT user_id, content, created_at
            FROM chat_records
            WHERE room_id = ?
            ORDER BY created_at ASC
        ''', (room_id,))
        
        chat_records = []
        for row in cursor.fetchall():
            chat_records.append({
                "user_id": row[0],
                "content": row[1],
                "created_at": row[2]
            })
        
        # 获取会话记录
        cursor.execute('''
            SELECT user_id, join_time, leave_time, duration_seconds
            FROM session_records
            WHERE room_id = ?
            ORDER BY join_time ASC
        ''', (room_id,))
        
        session_records = []
        for row in cursor.fetchall():
            session_records.append({
                "user_id": row[0],
                "join_time": row[1],
                "leave_time": row[2],
                "duration_seconds": row[3]
            })
        
        conn.close()
        
        return jsonify({
            "room_id": room_id,
            "sync_time": room_data[0],
            "admin_user_ids": json.loads(room_data[1]),
            "start_time": room_data[2],
            "stats": {
                "current_users": room_data[3],
                "peak_users": room_data[4],
                "total_joins": room_data[5]
            },
            "chat_history": chat_records,
            "session_history": session_records,
            "raw_data": json.loads(room_data[6])
        })
        
    except Exception as e:
        logger.error(f"获取房间详情时出错: {str(e)}")
        return jsonify({"error": f"获取失败: {str(e)}"}), 500

@app.route('/stats', methods=['GET'])
def get_stats():
    """获取统计信息"""
    try:
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        
        # 总房间数
        cursor.execute('SELECT COUNT(DISTINCT room_id) FROM room_syncs')
        total_rooms = cursor.fetchone()[0]
        
        # 总聊天消息数
        cursor.execute('SELECT COUNT(*) FROM chat_records')
        total_messages = cursor.fetchone()[0]
        
        # 总会话数
        cursor.execute('SELECT COUNT(*) FROM session_records')
        total_sessions = cursor.fetchone()[0]
        
        # 最近同步的房间
        cursor.execute('''
            SELECT room_id, sync_time
            FROM room_syncs
            ORDER BY sync_time DESC
            LIMIT 5
        ''')
        
        recent_syncs = []
        for row in cursor.fetchall():
            recent_syncs.append({
                "room_id": row[0],
                "sync_time": row[1]
            })
        
        conn.close()
        
        return jsonify({
            "total_rooms": total_rooms,
            "total_messages": total_messages,
            "total_sessions": total_sessions,
            "recent_syncs": recent_syncs
        })
        
    except Exception as e:
        logger.error(f"获取统计信息时出错: {str(e)}")
        return jsonify({"error": f"获取失败: {str(e)}"}), 500

if __name__ == '__main__':
    # 初始化数据库
    init_database()
    
    # 启动服务器
    logger.info("启动回调服务器...")
    app.run(host='0.0.0.0', port=8080, debug=True) 