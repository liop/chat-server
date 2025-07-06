#!/usr/bin/env python3
"""
测试拆分后的同步功能
"""

import requests
import json
import time
from datetime import datetime, timedelta

# 服务器配置
BASE_URL = "http://localhost:3000"
API_KEY = "test_key_123"

def test_room_creation():
    """测试房间创建"""
    print("=== 测试房间创建 ===")
    
    # 创建房间
    room_data = {
        "room_name": "测试拆分房间",
        "admin_user_ids": ["admin1", "admin2"]
    }
    
    response = requests.post(
        f"{BASE_URL}/management/rooms",
        headers={
            "Content-Type": "application/json",
            "X-Api-Key": API_KEY
        },
        json=room_data
    )
    
    if response.status_code == 200:
        room = response.json()
        print(f"✅ 房间创建成功: (ID: {room['room_id']})")
        print(f"  - WebSocket URL: {room['websocket_url']}")
        return room['room_id']
    else:
        print(f"❌ 房间创建失败: {response.status_code} - {response.text}")
        return None

def test_rooms_sync():
    """测试房间基础信息同步"""
    print("\n=== 测试房间基础信息同步 ===")
    
    response = requests.get(
        f"{BASE_URL}/management/sync/rooms",
        headers={"X-Api-Key": API_KEY}
    )
    
    if response.status_code == 200:
        rooms = response.json()
        print(f"✅ 获取到 {len(rooms)} 个房间的基础信息")
        for room in rooms:
            print(f"  - {room['room_name']}: {room['current_connections']} 用户在线")
        return True
    else:
        print(f"❌ 获取房间信息失败: {response.status_code} - {response.text}")
        return False

def test_chat_history_sync(room_id):
    """测试聊天记录同步"""
    print(f"\n=== 测试聊天记录同步 (房间: {room_id}) ===")
    
    # 计算时间范围（最近1小时）
    end_time = int(time.time())
    start_time = end_time - 3600
    
    params = {
        "page": 1,
        "limit": 100,
        "from": start_time,
        "to": end_time
    }
    
    response = requests.get(
        f"{BASE_URL}/management/sync/chat-history/{room_id}",
        headers={"X-Api-Key": API_KEY},
        params=params
    )
    
    if response.status_code == 200:
        data = response.json()
        print(f"✅ 聊天记录同步成功")
        print(f"  - 总记录数: {data['pagination']['total_records']}")
        print(f"  - 当前页: {data['pagination']['current_page']}")
        print(f"  - 每页大小: {data['pagination']['page_size']}")
        print(f"  - 总页数: {data['pagination']['total_pages']}")
        print(f"  - 实际返回: {len(data['records'])} 条记录")
        return True
    else:
        print(f"❌ 聊天记录同步失败: {response.status_code} - {response.text}")
        return False

def test_session_history_sync(room_id):
    """测试会话历史同步"""
    print(f"\n=== 测试会话历史同步 (房间: {room_id}) ===")
    
    # 计算时间范围（最近24小时）
    end_time = int(time.time())
    start_time = end_time - 86400
    
    params = {
        "page": 1,
        "limit": 50,
        "from": start_time,
        "to": end_time
    }
    
    response = requests.get(
        f"{BASE_URL}/management/sync/session-history/{room_id}",
        headers={"X-Api-Key": API_KEY},
        params=params
    )
    
    if response.status_code == 200:
        data = response.json()
        print(f"✅ 会话历史同步成功")
        print(f"  - 总会话数: {data['pagination']['total_records']}")
        print(f"  - 当前页: {data['pagination']['current_page']}")
        print(f"  - 每页大小: {data['pagination']['page_size']}")
        print(f"  - 总页数: {data['pagination']['total_pages']}")
        print(f"  - 实际返回: {len(data['records'])} 个会话")
        return True
    else:
        print(f"❌ 会话历史同步失败: {response.status_code} - {response.text}")
        return False

def test_traditional_sync():
    """测试传统同步接口"""
    print("\n=== 测试传统同步接口 ===")
    
    response = requests.get(
        f"{BASE_URL}/management/sync",
        headers={"X-Api-Key": API_KEY}
    )
    
    if response.status_code == 200:
        data = response.json()
        print(f"✅ 传统同步接口成功")
        print(f"  - 房间数量: {len(data)}")
        return True
    else:
        print(f"❌ 传统同步接口失败: {response.status_code} - {response.text}")
        return False

def test_manual_sync():
    """测试手动触发同步"""
    print("\n=== 测试手动触发同步 ===")
    
    response = requests.post(
        f"{BASE_URL}/management/sync",
        headers={"X-Api-Key": API_KEY}
    )
    
    if response.status_code == 200:
        print("✅ 手动同步触发成功")
        return True
    else:
        print(f"❌ 手动同步触发失败: {response.status_code} - {response.text}")
        return False

def main():
    """主测试函数"""
    print("🚀 开始测试拆分后的同步功能")
    print(f"服务器地址: {BASE_URL}")
    print(f"API密钥: {API_KEY}")
    print("=" * 50)
    
    # 测试房间创建
    room_id = test_room_creation()
    if not room_id:
        print("❌ 无法继续测试，房间创建失败")
        return
    
    # 等待一下让系统处理
    time.sleep(1)
    
    # 测试各种同步接口
    test_rooms_sync()
    test_chat_history_sync(room_id)
    test_session_history_sync(room_id)
    test_traditional_sync()
    test_manual_sync()
    
    print("\n" + "=" * 50)
    print("🎉 测试完成！")
    print("\n📝 测试说明:")
    print("1. 房间创建测试 - 验证房间创建和自动同步")
    print("2. 房间基础信息同步 - 验证 /management/sync/rooms 接口")
    print("3. 聊天记录同步 - 验证分页获取聊天记录")
    print("4. 会话历史同步 - 验证分页获取会话历史")
    print("5. 传统同步接口 - 验证向后兼容性")
    print("6. 手动同步触发 - 验证手动同步功能")

if __name__ == "__main__":
    main() 