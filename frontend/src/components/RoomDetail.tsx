import React from 'react';
import { RoomBasicInfo } from '../api';

interface RoomDetailProps {
  room: RoomBasicInfo | null;
}

const RoomDetail: React.FC<RoomDetailProps> = ({ room }) => {
  if (!room) return <div style={{ padding: 24 }}>请选择房间</div>;
  return (
    <div style={{ padding: 24 }}>
      <h2>{room.room_name}</h2>
      <div>房间ID: {room.room_id}</div>
      <div>管理员: {room.admin_user_ids.join(', ')}</div>
      <div>当前在线人数: {room.current_connections}</div>
      <div>创建时间: {new Date(room.created_at * 1000).toLocaleString()}</div>
      <div>最后活跃: {new Date(room.last_activity * 1000).toLocaleString()}</div>
    </div>
  );
};

export default RoomDetail; 