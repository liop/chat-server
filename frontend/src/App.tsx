import React, { useEffect, useState } from 'react';
import { fetchRooms, sendRoomMessage, RoomBasicInfo, createRoom } from './api';
import RoomList from './components/RoomList';
import RoomDetail from './components/RoomDetail';
import MessageSender from './components/MessageSender';

const API_KEY = 'test_key_123'; // TODO: 可做成输入框

const App: React.FC = () => {
  const [rooms, setRooms] = useState<RoomBasicInfo[]>([]);
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const loadRooms = async () => {
    setLoading(true);
    setError('');
    try {
      const data = await fetchRooms(API_KEY);
      setRooms(data);
    } catch (e) {
      setError('获取房间列表失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadRooms();
    const timer = setInterval(loadRooms, 5000);
    return () => clearInterval(timer);
  }, []);

  const selectedRoom = rooms.find(r => r.room_id === selectedRoomId) || null;

  const handleSend = async (content: string) => {
    if (!selectedRoomId) return;
    try {
      await sendRoomMessage(selectedRoomId, content, API_KEY);
      alert('发送成功');
    } catch {
      alert('发送失败');
    }
  };

  // 新建房间逻辑
  const handleCreateRoom = async (roomName: string, adminUserIds: string[]) => {
    if (!roomName || !roomName.trim()) {
      setError('房间名不能为空');
      return;
    }
    if (!adminUserIds || adminUserIds.length === 0) {
      setError('管理员ID不能为空');
      return;
    }
    setLoading(true);
    setError('');
    try {
      const newRoom = await createRoom(roomName.trim(), adminUserIds, API_KEY);
      await loadRooms();
      setSelectedRoomId(newRoom.room_id);
    } catch (e) {
      setError('新建房间失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ display: 'flex', height: '100vh' }}>
      <RoomList
        rooms={rooms}
        selectedRoomId={selectedRoomId}
        onSelect={setSelectedRoomId}
        onCreateRoom={handleCreateRoom}
      />
      <div style={{ flex: 1 }}>
        {loading && <div style={{ padding: 24 }}>加载中...</div>}
        {error && <div style={{ color: 'red', padding: 24 }}>{error}</div>}
        <RoomDetail room={selectedRoom} />
        {selectedRoom && <MessageSender onSend={handleSend} />}
      </div>
    </div>
  );
};

export default App; 