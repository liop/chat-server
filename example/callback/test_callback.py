#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
测试房间信息同步回调系统
演示如何启动回调服务器并测试数据同步
"""

import requests
import json
import time
import subprocess
import sys
import os
from datetime import datetime

# 回调服务器配置
CALLBACK_SERVER_URL = "http://localhost:8080"
RUST_SERVER_URL = "http://localhost:3000"

def test_callback_server():
    """测试回调服务器功能"""
    print("=== 测试回调服务器 ===")
    
    # 1. 健康检查
    try:
        response = requests.get(f"{CALLBACK_SERVER_URL}/health")
        if response.status_code == 200:
            print("✅ 回调服务器健康检查通过")
        else:
            print("❌ 回调服务器健康检查失败")
            return False
    except requests.exceptions.ConnectionError:
        print("❌ 无法连接到回调服务器，请确保服务器已启动")
        return False
    
    # 2. 模拟房间数据同步
    mock_room_data = {
        "room_id": "550e8400-e29b-41d4-a716-446655440000",
        "admin_user_ids": ["admin1", "admin2"],
        "start_time": int(time.time()) - 3600,  # 1小时前
        "stats": {
            "current_users": 5,
            "peak_users": 15,
            "total_joins": 25
        },
        "chat_history": [
            {
                "user_id": "user1",
                "content": "大家好！",
                "created_at": int(time.time()) - 1800
            },
            {
                "user_id": "user2", 
                "content": "你好！",
                "created_at": int(time.time()) - 1700
            },
            {
                "user_id": "admin1",
                "content": "欢迎来到房间！",
                "created_at": int(time.time()) - 1600
            }
        ],
        "session_history": [
            {
                "user_id": "user1",
                "join_time": int(time.time()) - 3600,
                "leave_time": int(time.time()) - 1800,
                "duration_seconds": 1800
            },
            {
                "user_id": "user2",
                "join_time": int(time.time()) - 3500,
                "leave_time": int(time.time()) - 1700,
                "duration_seconds": 1800
            }
        ]
    }
    
    try:
        response = requests.post(
            f"{CALLBACK_SERVER_URL}/sync/room",
            json=mock_room_data,
            headers={"Content-Type": "application/json"}
        )
        
        if response.status_code == 200:
            print("✅ 房间数据同步测试成功")
            result = response.json()
            print(f"   响应: {result.get('message', '')}")
        else:
            print(f"❌ 房间数据同步测试失败: {response.status_code}")
            print(f"   错误: {response.text}")
            return False
    except Exception as e:
        print(f"❌ 房间数据同步测试异常: {str(e)}")
        return False
    
    # 3. 验证数据是否正确保存
    time.sleep(1)  # 等待数据库写入
    
    try:
        response = requests.get(f"{CALLBACK_SERVER_URL}/rooms")
        if response.status_code == 200:
            rooms = response.json().get("rooms", [])
            if rooms:
                print("✅ 房间列表查询成功")
                print(f"   找到 {len(rooms)} 个房间")
                
                # 获取第一个房间的详细信息
                room_id = rooms[0]["room_id"]
                detail_response = requests.get(f"{CALLBACK_SERVER_URL}/rooms/{room_id}")
                if detail_response.status_code == 200:
                    room_detail = detail_response.json()
                    print("✅ 房间详情查询成功")
                    print(f"   房间ID: {room_detail['room_id']}")
                    print(f"   管理员: {room_detail['admin_user_ids']}")
                    print(f"   聊天记录数: {len(room_detail['chat_history'])}")
                    print(f"   会话记录数: {len(room_detail['session_history'])}")
                else:
                    print("❌ 房间详情查询失败")
            else:
                print("❌ 未找到同步的房间数据")
                return False
        else:
            print("❌ 房间列表查询失败")
            return False
    except Exception as e:
        print(f"❌ 数据验证异常: {str(e)}")
        return False
    
    # 4. 获取统计信息
    try:
        response = requests.get(f"{CALLBACK_SERVER_URL}/stats")
        if response.status_code == 200:
            stats = response.json()
            print("✅ 统计信息查询成功")
            print(f"   总房间数: {stats['total_rooms']}")
            print(f"   总消息数: {stats['total_messages']}")
            print(f"   总会话数: {stats['total_sessions']}")
        else:
            print("❌ 统计信息查询失败")
    except Exception as e:
        print(f"❌ 统计信息查询异常: {str(e)}")
    
    return True

def test_rust_server_integration():
    """测试与Rust服务器的集成"""
    print("\n=== 测试Rust服务器集成 ===")
    
    # 检查Rust服务器是否运行
    try:
        response = requests.get(f"{RUST_SERVER_URL}/health")
        if response.status_code == 200:
            print("✅ Rust服务器运行正常")
        else:
            print("❌ Rust服务器响应异常")
            return False
    except requests.exceptions.ConnectionError:
        print("❌ 无法连接到Rust服务器，请确保服务器已启动")
        return False
    
    # 创建房间
    create_room_data = {
        "room_name": "测试房间",
        "admin_user_ids": ["admin1", "admin2"]
    }
    
    try:
        response = requests.post(
            f"{RUST_SERVER_URL}/rooms",
            json=create_room_data,
            headers={
                "Content-Type": "application/json",
                "X-Api-Key": "your-secret-admin-key-here"  # 使用配置文件中的API密钥
            }
        )
        
        if response.status_code == 200:
            room_info = response.json()
            room_id = room_info["room_id"]
            print(f"✅ 房间创建成功: {room_id}")
            
            # 关闭房间（这会触发数据同步）
            close_response = requests.delete(
                f"{RUST_SERVER_URL}/rooms/{room_id}",
                headers={"X-Api-Key": "your-secret-admin-key-here"}
            )
            
            if close_response.status_code == 204:
                print("✅ 房间关闭成功，数据同步已触发")
                
                # 等待数据同步完成
                time.sleep(2)
                
                # 检查回调服务器是否收到数据
                stats_response = requests.get(f"{CALLBACK_SERVER_URL}/stats")
                if stats_response.status_code == 200:
                    stats = stats_response.json()
                    if stats["total_rooms"] > 0:
                        print("✅ 数据同步验证成功")
                        return True
                    else:
                        print("❌ 数据同步验证失败：未找到同步的房间")
                        return False
                else:
                    print("❌ 无法验证数据同步")
                    return False
            else:
                print(f"❌ 房间关闭失败: {close_response.status_code}")
                return False
        else:
            print(f"❌ 房间创建失败: {response.status_code}")
            print(f"   错误: {response.text}")
            return False
    except Exception as e:
        print(f"❌ Rust服务器集成测试异常: {str(e)}")
        return False

def main():
    """主函数"""
    print("🚀 房间信息同步回调系统测试")
    print("=" * 50)
    
    # 检查回调服务器是否运行
    print("1. 检查回调服务器状态...")
    try:
        response = requests.get(f"{CALLBACK_SERVER_URL}/health", timeout=5)
        if response.status_code != 200:
            print("❌ 回调服务器未正常运行")
            print("请先启动回调服务器: python callback_server.py")
            return
    except requests.exceptions.ConnectionError:
        print("❌ 回调服务器未运行")
        print("请先启动回调服务器: python callback_server.py")
        return
    
    # 测试回调服务器功能
    print("\n2. 测试回调服务器功能...")
    if not test_callback_server():
        print("❌ 回调服务器功能测试失败")
        return
    
    # 测试Rust服务器集成
    print("\n3. 测试Rust服务器集成...")
    if not test_rust_server_integration():
        print("❌ Rust服务器集成测试失败")
        return
    
    print("\n🎉 所有测试通过！")
    print("\n使用说明:")
    print("1. 启动回调服务器: python callback_server.py")
    print("2. 启动Rust服务器: cargo run")
    print("3. 配置Rust服务器的config.toml文件，设置data_callback_url")
    print("4. 当房间关闭时，数据会自动同步到回调服务器")

if __name__ == "__main__":
    main() 