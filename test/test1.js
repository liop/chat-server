import ws from 'k6/ws';
import http from 'k6/http';
import { check, sleep } from 'k6';
import exec from 'k6/execution';


// 测试配置
export const options = {
  // 定义虚拟用户（VUs）和测试时长
  stages: [
    { duration: '1m', target: 100 }, // 1分钟内，用户数从0线性增加到100
    { duration: '3m', target: 100 }, // 保持100个用户运行3分钟
    { duration: '1m', target: 0 }, // 1分钟内，用户数从100线性减少到0
  ],
  thresholds: {
    // 定义性能断言（PNC）
    ws_session_duration: ['p(95)<5000'], // 95%的会话持续时间应小于5秒 (这里只是示例，根据你的场景调整)
    ws_msgs_sent: ['count>0'],
  },
};



export function setup() {
  const api_key = 'test_key_123';

  const res = http.post(
    'http://localhost:3000/management/rooms',
    JSON.stringify({
      room_name: 'test_room_'+ Date.now(),
      admin_user_ids: ['test_admin'],
    }),
    {
      headers: {
        'X-Api-Key': `${api_key}`,
        'Content-Type': 'application/json'
      },
    }
  );

  console.log('setup res:', res.json());

  return res.json()

}

export default function (data) {
   
   
  const { roomId } = data; // 所有用户加入同一个房间
  const userId = `user_${__VU}`; // 使用 k6 的虚拟用户ID确保唯一性
  const nickname = `k6_user_${__VU}`; // 每个虚拟用户有不同的昵称


  const url = `ws://localhost:3000/ws/rooms/${roomId}?user_id=${userId}&nickname=${nickname}`;

  // 建立WebSocket连接
  const res = ws.connect(url, {}, function (socket) {
    // 1. 连接成功时的回调函数
    socket.on('open', function open() {
      console.log(`VU ${__VU}: WebSocket connection established!`);

      // 1.1 加入房间 (模拟发送一个JSON消息)
      socket.send(
        JSON.stringify({
          type: 'SendMessage',
          payload: { content: `user_${__VU}` },
        })
      );

      // 1.2 设置一个定时器，每10秒发送一次心跳
      socket.setInterval(function timeout() {
        socket.send(JSON.stringify({ type: 'Ping' }));
        console.log(`VU ${__VU}: Pinging...`);
      }, 10000); // 10秒

      // 1.3 设置一个定时器，模拟用户随机发送聊天消息
      socket.setInterval(function sendMessage() {
        socket.send(
          JSON.stringify({
            type: 'SendMessage',
            payload: { content: `Hello from VU ${__VU}` },
          })
        );
      }, Math.random() * (15000 - 5000) + 5000); // 5到15秒随机发送一条消息
    });

    // 2. 收到消息时的回调函数
    socket.on('message', function (data) {
      const msg = JSON.parse(data);
      // 检查收到的消息是否是pong或广播消息
      if (msg.type === 'Pong') {
        console.log(`VU ${__VU}: Received pong`);
      } else {
        console.log(
          `VU ${__VU}: Received broadcast message: ${msg.payload.content}`
        );
      }
      // 可以用 check 来验证消息内容
      check(data, {
        'message is not empty': (d) => d.length > 0,
      });
    });

    // 3. 连接关闭时的回调函数
    socket.on('close', function () {
      console.log(`VU ${__VU}: WebSocket connection closed.`);
    });

    // 4. 发生错误时的回调函数
    socket.on('error', function (e) {
      console.error(`An error occurred: ${e.error()}`);
    });

    // 5. 在整个会话结束后（例如30秒后），主动关闭连接
    socket.setTimeout(function () {
      console.log(`VU ${__VU}: Closing the connection after 30s.`);
      socket.close();
    }, 30000);
  });

  // 检查连接是否成功建立
  check(res, { 'status is 101': (r) => r && r.status === 101 });

  sleep(1); // 等待1秒，让主函数不至于过快结束
}
