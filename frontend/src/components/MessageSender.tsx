import React, { useState } from 'react';

interface MessageSenderProps {
  onSend: (content: string) => void;
}

const MessageSender: React.FC<MessageSenderProps> = ({ onSend }) => {
  const [content, setContent] = useState('');
  return (
    <div style={{ marginTop: 24 }}>
      <input
        type="text"
        value={content}
        onChange={e => setContent(e.target.value)}
        placeholder="输入要发送的消息"
        style={{ width: 300, marginRight: 8 }}
      />
      <button onClick={() => { if (content.trim()) { onSend(content); setContent(''); } }}>发送</button>
    </div>
  );
};

export default MessageSender; 