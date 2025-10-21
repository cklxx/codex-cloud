"use client";

import { useEffect, useState } from "react";
import { Button, Card, Form, Input, Select, Typography, message } from "antd";
import { useRouter } from "next/router";
import { AuthGuard } from "@/components/AuthGuard";
import { AppLayout } from "@/components/AppLayout";
import { useAuth } from "@/contexts/AuthContext";
import { apiFetchAuthed } from "@/lib/api";

interface RepositoryOption {
  id: string;
  name: string;
}

export default function TaskCreatePage() {
  const router = useRouter();
  const { token } = useAuth();
  const [repositories, setRepositories] = useState<RepositoryOption[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const loadRepositories = async () => {
      if (!token) return;
      try {
        const response = await apiFetchAuthed("/repositories", token);
        const data = (await response.json()) as RepositoryOption[];
        setRepositories(data);
      } catch (error) {
        console.error(error);
        message.error("加载仓库列表失败");
      }
    };
    void loadRepositories();
  }, [token]);

  const handleCreate = async (values: { title: string; description?: string; repository_id: string }) => {
    if (!token) return;
    try {
      setLoading(true);
      const response = await apiFetchAuthed("/tasks", token, {
        method: "POST",
        body: JSON.stringify(values)
      });
      if (response.ok) {
        const data = (await response.json()) as { id: string };
        message.success("任务已创建");
        void router.push(`/tasks/${data.id}`);
      } else {
        message.error("创建任务失败");
      }
    } catch (error) {
      console.error(error);
      message.error("创建任务失败，请稍后重试");
    } finally {
      setLoading(false);
    }
  };

  return (
    <AuthGuard>
      <AppLayout>
        <Card>
          <Typography.Title level={4}>创建任务</Typography.Title>
          <Form layout="vertical" onFinish={handleCreate}>
            <Form.Item
              name="title"
              label="标题"
              rules={[{ required: true, message: "请输入任务标题" }]}
            >
              <Input placeholder="例如：实现快速启动执行器" />
            </Form.Item>
            <Form.Item name="description" label="描述">
              <Input.TextArea rows={6} placeholder="补充任务背景、验收标准等信息" />
            </Form.Item>
            <Form.Item
              name="repository_id"
              label="仓库"
              rules={[{ required: true, message: "请选择目标仓库" }]}
            >
              <Select
                placeholder="选择仓库"
                options={repositories.map((repo) => ({ label: repo.name, value: repo.id }))}
              />
            </Form.Item>
            <Button type="primary" htmlType="submit" loading={loading}>
              创建任务
            </Button>
          </Form>
        </Card>
      </AppLayout>
    </AuthGuard>
  );
}
