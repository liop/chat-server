#!/usr/bin/env python3
"""
数据同步功能测试脚本
演示如何使用Rust聊天服务器的同步功能
"""

import requests
import json
import time
import uuid

# 配置
SERVER_URL = "http://localhost:3000"
API_KEY = "your_secret_api_key_here"  # 请替换为实际的API密钥

headers = {
    "Content-Type": "application/json",
    "X-Api-Key": API_KEY
}

def test_create_room_with_sync():
    """测试创建房间时的自动同步"""
    print("=== 测试创建房间时的自动同步 ===")
    
    room_data = {
        "room_name": f"测试房间_{uuid.uuid4().hex[:8]}",
        "admin_user_ids": ["admin1", "admin2"]
    }
    
    try:
        response = requests.post(
            f"{SERVER_URL}/management/rooms",
            headers=headers,
            json=room_data
        )
        
        if response.status_code == 200:
            result = response.json()
            print(f"✅ 房间创建成功: {result['room_id']}")
            print(f"   WebSocket URL: {result['websocket_url']}")
            print("   📤 系统会自动触发数据同步")
            return result['room_id']
        else:
            print(f"❌ 房间创建失败: {response.status_code}")
            print(f"   错误信息: {response.text}")
            return None
            
    except Exception as e:
        print(f"❌ 请求失败: {e}")
        return None

def test_manual_sync():
    """测试手动触发同步"""
    print("\n=== 测试手动触发同步 ===")
    
    try:
        response = requests.post(
            f"{SERVER_URL}/management/sync",
            headers=headers
        )
        
        if response.status_code == 202:
            print("✅ 手动同步已触发")
            print("   📤 系统正在后台同步所有房间数据")
        else:
            print(f"❌ 手动同步失败: {response.status_code}")
            print(f"   错误信息: {response.text}")
            
    except Exception as e:
        print(f"❌ 请求失败: {e}")

def test_get_sync_data():
    """测试获取同步数据"""
    print("\n=== 测试获取同步数据 ===")
    
    try:
        response = requests.get(
            f"{SERVER_URL}/management/sync",
            headers=headers
        )
        
        if response.status_code == 200:
            sync_data = response.json()
            print(f"✅ 成功获取同步数据")
            print(f"   房间数量: {len(sync_data)}")
            
            for room in sync_data:
                print(f"   📋 房间: {room['room_id']}")
                print(f"      名称: {room['room_name']}")
                print(f"      连接数: {room['current_connections']}")
                print(f"      管理员: {room['admin_user_ids']}")
                print(f"      封禁用户: {room['banned_user_ids']}")
                print()
        else:
            print(f"❌ 获取同步数据失败: {response.status_code}")
            print(f"   错误信息: {response.text}")
            
    except Exception as e:
        print(f"❌ 请求失败: {e}")

def test_list_rooms():
    """测试获取房间列表"""
    print("\n=== 测试获取房间列表 ===")
    
    try:
        response = requests.get(
            f"{SERVER_URL}/management/rooms",
            headers=headers
        )
        
        if response.status_code == 200:
            rooms = response.json()
            print(f"✅ 成功获取房间列表")
            print(f"   房间数量: {len(rooms)}")
            
            for room in rooms:
                print(f"   📋 房间: {room['room_id']}")
                print(f"      名称: {room['room_name']}")
                print(f"      连接数: {room['current_connections']}")
                print()
        else:
            print(f"❌ 获取房间列表失败: {response.status_code}")
            print(f"   错误信息: {response.text}")
            
    except Exception as e:
        print(f"❌ 请求失败: {e}")

def main():
    """主测试函数"""
    print("🚀 Rust聊天服务器 - 数据同步功能测试")
    print("=" * 50)
    
    # 检查服务器是否运行
    try:
        health_response = requests.get(f"{SERVER_URL}/management/health")
        if health_response.status_code != 200:
            print("❌ 服务器未运行或无法访问")
            return
        print("✅ 服务器运行正常")
    except Exception as e:
        print(f"❌ 无法连接到服务器: {e}")
        return
    
    # 创建测试房间
    room_id = test_create_room_with_sync()
    
    if room_id:
        # 等待一下让同步完成
        time.sleep(2)
        
        # 测试手动同步
        test_manual_sync()
        
        # 等待同步完成
        time.sleep(2)
        
        # 测试获取同步数据
        test_get_sync_data()
        
        # 测试获取房间列表
        test_list_rooms()
    
    print("\n" + "=" * 50)
    print("🎉 测试完成！")
    print("\n📝 同步功能说明:")
    print("1. 创建房间时自动同步 - 每次创建房间都会触发一次数据同步")
    print("2. 定时同步 - 根据SYNC_INTERVAL_SECONDS配置定期同步")
    print("3. 手动同步 - 通过POST /management/sync手动触发")
    print("4. 主动拉取 - 通过GET /management/sync获取所有房间数据")

if __name__ == "__main__":
    main() 