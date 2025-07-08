import axios from 'axios';

const API_BASE = '/api';

export interface RoomBasicInfo {
  room_id: string;
  room_name: string;
  admin_user_ids: string[];
  current_connections: number;
  created_at: number;
  last_activity: number;
}

export async function fetchRooms(apiKey: string): Promise<RoomBasicInfo[]> {
  const res = await axios.get(`${API_BASE}/management/rooms`, {
    headers: { 'X-Api-Key': apiKey },
  });
  return res.data;
}

export async function sendRoomMessage(roomId: string, content: string, apiKey: string) {
  // 假设有管理API用于发送系统消息
  return axios.post(
    `${API_BASE}/management/rooms/${roomId}/system-message`,
    { content },
    { headers: { 'X-Api-Key': apiKey } }
  );
}

export async function createRoom(
  roomName: string,
  adminUserIds: string[],
  apiKey: string
) {
  const res = await axios.post(
    `${API_BASE}/management/rooms`,
    { room_name: roomName, admin_user_ids: adminUserIds },
    { headers: { 'X-Api-Key': apiKey } }
  );
  return res.data;
} 