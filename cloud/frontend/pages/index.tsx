"use client";

import { useEffect, useState } from "react";
import { Button, Card, Form, Input, Tabs, Typography, message } from "antd";
import { useRouter } from "next/router";
import { useAuth } from "@/contexts/AuthContext";

export default function HomePage() {
  const { token, loading, login, register } = useAuth();
  const router = useRouter();
  const [loginSubmitting, setLoginSubmitting] = useState(false);
  const [registerSubmitting, setRegisterSubmitting] = useState(false);

  useEffect(() => {
    if (!loading && token) {
      void router.replace("/tasks");
    }
  }, [token, loading, router]);

  const handleLogin = async (values: { email: string; password: string }) => {
    try {
      setLoginSubmitting(true);
      await login(values.email, values.password);
      void router.replace("/tasks");
    } catch (error) {
      console.error(error);
      message.error("登录失败，请检查邮箱与密码");
    } finally {
      setLoginSubmitting(false);
    }
  };

  const handleRegister = async (values: { email: string; password: string; name?: string }) => {
    try {
      setRegisterSubmitting(true);
      await register(values.email, values.password, values.name);
    } catch (error) {
      console.error(error);
      message.error("注册失败，请稍后重试");
    } finally {
      setRegisterSubmitting(false);
    }
  };

  return (
    <div
      style={{
        minHeight: "100vh",
        display: "flex",
        alignItems: "center",
        justifyContent: "center"
      }}
    >
      <Card style={{ width: 420 }}>
        <Typography.Title level={3} style={{ textAlign: "center" }}>
          Codex Cloud 登录
        </Typography.Title>
        <Tabs
          defaultActiveKey="login"
          items={[
            {
              key: "login",
              label: "登录",
              children: (
                <Form layout="vertical" onFinish={handleLogin}>
                  <Form.Item name="email" label="邮箱" rules={[{ required: true, message: "请输入邮箱" }]}> 
                    <Input type="email" placeholder="admin@example.com" />
                  </Form.Item>
                  <Form.Item name="password" label="密码" rules={[{ required: true, message: "请输入密码" }]}> 
                    <Input.Password placeholder="******" />
                  </Form.Item>
                  <Button block type="primary" htmlType="submit" loading={loginSubmitting}>
                    登录
                  </Button>
                </Form>
              )
            },
            {
              key: "register",
              label: "注册",
              children: (
                <Form layout="vertical" onFinish={handleRegister}>
                  <Form.Item name="email" label="邮箱" rules={[{ required: true, message: "请输入邮箱" }]}> 
                    <Input type="email" placeholder="admin@example.com" />
                  </Form.Item>
                  <Form.Item name="name" label="昵称">
                    <Input placeholder="Admin" />
                  </Form.Item>
                  <Form.Item
                    name="password"
                    label="密码"
                    rules={[{ required: true, message: "请输入密码" }]}
                  >
                    <Input.Password placeholder="******" />
                  </Form.Item>
                  <Button block type="primary" htmlType="submit" loading={registerSubmitting}>
                    注册
                  </Button>
                </Form>
              )
            }
          ]}
        />
      </Card>
    </div>
  );
}
