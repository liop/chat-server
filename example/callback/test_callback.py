#!/usr/bin/env python3
"""
房间信息同步回调系统测试脚本
测试拆分后的回调接口和传统接口
"""

import requests
import json
import time
import uuid
from datetime import datetime

# 配置
RUST_SERVER_URL = "http://localhost:3000"
CALLBACK_SERVER_URL = "http://localhost:8080"
API_KEY = "your_secret_api_key_here"  # 请替换为实际的API密钥

headers = {
    "Content-Type": "application/json",
    "X-Api-Key": API_KEY
}

def test_health_check():
    """测试健康检查"""
    print("=== 测试健康检查 ===")
    
    try:
        # 测试Rust服务器
        response = requests.get(f"{RUST_SERVER_URL}/management/health")
        if response.status_code == 200:
            print("✅ Rust服务器运行正常")
        else:
            print(f"❌ Rust服务器异常: {response.status_code}")
            return False
        
        # 测试回调服务器
        response = requests.get(f"{CALLBACK_SERVER_URL}/health")
        if response.status_code == 200:
            print("✅ 回调服务器运行正常")
        else:
            print(f"❌ 回调服务器异常: {response.status_code}")
            return False
            
        return True
        
    except Exception as e:
        print(f"❌ 健康检查失败: {e}")
        return False

def test_create_room_with_events():
    """测试创建房间并触发事件回调"""
    print("\n=== 测试创建房间事件回调 ===")
    
    room_data = {
        "room_name": f"测试房间_{uuid.uuid4().hex[:8]}",
        "admin_user_ids": ["admin1", "admin2"]
    }
    
    try:
        response = requests.post(
            f"{RUST_SERVER_URL}/management/rooms",
            headers=headers,
            json=room_data
        )
        
        if response.status_code == 200:
            result = response.json()
            room_id = result['room_id']
            print(f"✅ 房间创建成功: {room_id}")
            print(f"   WebSocket URL: {result['websocket_url']}")
            print("   📤 系统会自动触发房间创建事件回调")
            return room_id
        else:
            print(f"❌ 房间创建失败: {response.status_code}")
            print(f"   错误信息: {response.text}")
            return None
            
    except Exception as e:
        print(f"❌ 请求失败: {e}")
        return None

def test_split_sync_interfaces():
    """测试拆分后的同步接口"""
    print("\n=== 测试拆分后的同步接口 ===")
    
    try:
        # 1. 获取房间基础信息
        print("1. 测试获取房间基础信息...")
        response = requests.get(
            f"{RUST_SERVER_URL}/management/sync/rooms",
            headers=headers
        )
        
        if response.status_code == 200:
            rooms = response.json()
            print(f"   ✅ 成功获取 {len(rooms)} 个房间的基础信息")
            
            if rooms:
                room_id = rooms[0]['room_id']
                print(f"   📋 示例房间: {room_id}")
                print(f"      名称: {rooms[0]['room_name']}")
                print(f"      连接数: {rooms[0]['current_connections']}")
                print(f"      管理员: {rooms[0]['admin_user_ids']}")
                
                # 2. 测试获取聊天记录（分页）
                print("\n2. 测试获取聊天记录（分页）...")
                response = requests.get(
                    f"{RUST_SERVER_URL}/management/sync/chat-history/{room_id}",
                    headers=headers,
                    params={"page": 1, "limit": 10}
                )
                
                if response.status_code == 200:
                    chat_page = response.json()
                    print(f"   ✅ 成功获取聊天记录")
                    print(f"      房间ID: {chat_page['room_id']}")
                    print(f"      记录数: {len(chat_page['records'])}")
                    print(f"      分页信息: 第{chat_page['pagination']['current_page']}页，共{chat_page['pagination']['total_pages']}页")
                    print(f"      总记录数: {chat_page['pagination']['total_records']}")
                else:
                    print(f"   ❌ 获取聊天记录失败: {response.status_code}")
                
                # 3. 测试获取会话历史（分页）
                print("\n3. 测试获取会话历史（分页）...")
                response = requests.get(
                    f"{RUST_SERVER_URL}/management/sync/session-history/{room_id}",
                    headers=headers,
                    params={"page": 1, "limit": 10}
                )
                
                if response.status_code == 200:
                    session_page = response.json()
                    print(f"   ✅ 成功获取会话历史")
                    print(f"      房间ID: {session_page['room_id']}")
                    print(f"      记录数: {len(session_page['records'])}")
                    print(f"      分页信息: 第{session_page['pagination']['current_page']}页，共{session_page['pagination']['total_pages']}页")
                    print(f"      总记录数: {session_page['pagination']['total_records']}")
                else:
                    print(f"   ❌ 获取会话历史失败: {response.status_code}")
                
                return room_id
            else:
                print("   ⚠️ 没有找到房间，跳过分页测试")
                return None
        else:
            print(f"   ❌ 获取房间基础信息失败: {response.status_code}")
            return None
            
    except Exception as e:
        print(f"❌ 测试拆分接口失败: {e}")
        return None

def test_legacy_sync_interface():
    """测试传统同步接口（保持向后兼容）"""
    print("\n=== 测试传统同步接口 ===")
    
    try:
        response = requests.get(
            f"{RUST_SERVER_URL}/management/sync",
            headers=headers
        )
        
        if response.status_code == 200:
            sync_data = response.json()
            print(f"✅ 成功获取传统同步数据")
            print(f"   房间数量: {len(sync_data)}")
            
            for room in sync_data:
                print(f"   📋 房间: {room['room_id']}")
                print(f"      管理员: {room['admin_user_ids']}")
                print(f"      聊天记录: {len(room['chat_history'])}条")
                print(f"      会话记录: {len(room['session_history'])}条")
                print()
        else:
            print(f"❌ 获取传统同步数据失败: {response.status_code}")
            
    except Exception as e:
        print(f"❌ 测试传统接口失败: {e}")

def test_callback_server_stats():
    """测试回调服务器统计信息"""
    print("\n=== 测试回调服务器统计 ===")
    
    try:
        # 获取房间列表
        response = requests.get(f"{CALLBACK_SERVER_URL}/rooms")
        if response.status_code == 200:
            rooms = response.json()['rooms']
            print(f"✅ 回调服务器房间列表: {len(rooms)}个房间")
            
            if rooms:
                room_id = rooms[0]['room_id']
                print(f"   最新房间: {room_id}")
                print(f"   最后同步: {rooms[0]['last_sync_formatted']}")
                
                # 获取房间详情
                response = requests.get(f"{CALLBACK_SERVER_URL}/rooms/{room_id}")
                if response.status_code == 200:
                    details = response.json()
                    print(f"   📊 房间详情:")
                    print(f"      聊天记录数: {details['chat_count']}")
                    print(f"      会话记录数: {details['session_count']}")
                    print(f"      最近事件数: {len(details['recent_events'])}")
        
        # 获取统计信息
        response = requests.get(f"{CALLBACK_SERVER_URL}/stats")
        if response.status_code == 200:
            stats = response.json()
            print(f"\n📈 回调服务器统计:")
            print(f"   总房间数: {stats['total_rooms']}")
            print(f"   总聊天记录: {stats['total_chat_records']}")
            print(f"   总会话记录: {stats['total_session_records']}")
            print(f"   总事件数: {stats['total_events']}")
            print(f"   今日同步: {stats['today_syncs']}")
            
    except Exception as e:
        print(f"❌ 测试回调服务器统计失败: {e}")

def test_manual_sync_trigger():
    """测试手动触发同步"""
    print("\n=== 测试手动触发同步 ===")
    
    try:
        response = requests.post(
            f"{RUST_SERVER_URL}/management/sync",
            headers=headers
        )
        
        if response.status_code == 202:
            print("✅ 手动同步已触发")
            print("   📤 系统正在后台同步所有房间数据")
            
            # 等待一下让同步完成
            time.sleep(3)
            
            # 检查回调服务器是否有新的同步记录
            response = requests.get(f"{CALLBACK_SERVER_URL}/stats")
            if response.status_code == 200:
                stats = response.json()
                print(f"   📊 当前统计: 今日同步 {stats['today_syncs']} 次")
        else:
            print(f"❌ 手动同步失败: {response.status_code}")
            
    except Exception as e:
        print(f"❌ 测试手动同步失败: {e}")

def test_close_room_with_events():
    """测试关闭房间并触发事件回调"""
    print("\n=== 测试关闭房间事件回调 ===")
    
    # 先创建一个房间
    room_id = test_create_room_with_events()
    if not room_id:
        print("❌ 无法创建房间进行关闭测试")
        return
    
    # 等待一下
    time.sleep(2)
    
    try:
        response = requests.delete(
            f"{RUST_SERVER_URL}/management/rooms/{room_id}",
            headers=headers
        )
        
        if response.status_code == 204:
            print(f"✅ 房间关闭成功: {room_id}")
            print("   📤 系统会自动触发房间关闭事件回调")
            
            # 等待一下让回调完成
            time.sleep(3)
            
            # 检查回调服务器是否有房间关闭事件
            response = requests.get(f"{CALLBACK_SERVER_URL}/rooms/{room_id}")
            if response.status_code == 200:
                details = response.json()
                print(f"   📊 回调记录: {len(details['recent_events'])} 个事件")
        else:
            print(f"❌ 房间关闭失败: {response.status_code}")
            
    except Exception as e:
        print(f"❌ 测试关闭房间失败: {e}")

def main():
    """主测试函数"""
    print("🚀 房间信息同步回调系统 - 完整测试")
    print("=" * 60)
    
    # 1. 健康检查
    if not test_health_check():
        print("❌ 服务器未运行，请先启动Rust服务器和回调服务器")
        return
    
    # 2. 测试创建房间事件
    room_id = test_create_room_with_events()
    
    # 3. 测试拆分后的同步接口
    test_split_sync_interfaces()
    
    # 4. 测试传统同步接口
    test_legacy_sync_interface()
    
    # 5. 测试手动触发同步
    test_manual_sync_trigger()
    
    # 6. 测试回调服务器统计
    test_callback_server_stats()
    
    # 7. 测试关闭房间事件
    test_close_room_with_events()
    
    print("\n" + "=" * 60)
    print("🎉 测试完成！")
    print("\n📝 功能说明:")
    print("1. 房间事件回调 - 实时房间创建、关闭、用户加入/离开事件")
    print("2. 聊天记录批次回调 - 支持大数据量的分页传输")
    print("3. 会话历史批次回调 - 支持大数据量的分页传输")
    print("4. 定时同步回调 - 定期同步房间基础信息")
    print("5. 传统同步接口 - 保持向后兼容的完整数据同步")
    print("6. 拆分后的查询接口 - 支持分页查询聊天记录和会话历史")
    print("\n🔧 配置说明:")
    print("- 传统接口: DATA_CALLBACK_URL")
    print("- 房间事件: ROOM_EVENT_CALLBACK_URL")
    print("- 聊天记录: CHAT_HISTORY_CALLBACK_URL")
    print("- 会话历史: SESSION_HISTORY_CALLBACK_URL")
    print("- 定时同步: PERIODIC_SYNC_CALLBACK_URL")

if __name__ == "__main__":
    main() 