import React from 'react';
import { RoomBasicInfo } from '../api';

interface RoomListProps {
  rooms: RoomBasicInfo[];
  selectedRoomId: string | null;
  onSelect: (roomId: string) => void;
  onCreateRoom: (roomName: string, adminUserIds: string[]) => void; // 修改
}

const RoomList: React.FC<RoomListProps> = ({ rooms, selectedRoomId, onSelect, onCreateRoom }) => {
  // 新建房间处理函数
  const handleCreateRoom = () => {
    const roomName = window.prompt('请输入新房间名称');
    if (!roomName || !roomName.trim()) return;
    const adminUserIdsStr = window.prompt('请输入管理员用户ID（多个用英文逗号分隔）');
    if (!adminUserIdsStr || !adminUserIdsStr.trim()) return;
    const adminUserIds = adminUserIdsStr.split(',').map(id => id.trim()).filter(Boolean);
    if (adminUserIds.length === 0) return;
    onCreateRoom(roomName.trim(), adminUserIds);
  };

  return (
    <div style={{ width: 300, borderRight: '1px solid #eee', height: '100vh', overflowY: 'auto' }}>
      <h3 style={{ textAlign: 'center' }}>
        房间列表
        <button
          style={{
            marginLeft: 10,
            fontSize: 14,
            padding: '2px 8px',
            cursor: 'pointer',
            borderRadius: 4,
            border: '1px solid #1890ff',
            background: '#1890ff',
            color: '#fff'
          }}
          onClick={handleCreateRoom}
        >
          新建房间
        </button>
      </h3>
      <ul style={{ listStyle: 'none', padding: 0 }}>
        {rooms.map(room => (
          <li
            key={room.room_id}
            style={{
              padding: 12,
              background: selectedRoomId === room.room_id ? '#e6f7ff' : undefined,
              cursor: 'pointer',
              borderBottom: '1px solid #f0f0f0',
            }}
            onClick={() => onSelect(room.room_id)}
          >
            <div><b>{room.room_name}</b></div>
            <div style={{ fontSize: 12, color: '#888' }}>在线人数: {room.current_users}</div>
            <div style={{ fontSize: 12, color: '#888' }}>管理员: {room.admin_user_ids.join(', ')}</div>
          </li>
        ))}
      </ul>
    </div>
  );
};

export default RoomList; 