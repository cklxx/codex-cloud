"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { Badge, Button, Card, Flex, Select, Space, Table, Tag, Typography, message } from "antd";
import type { ColumnsType } from "antd/es/table";
import Link from "next/link";
import dayjs from "dayjs";
import { useAuth } from "@/contexts/AuthContext";
import { apiFetchAuthed } from "@/lib/api";
import { AuthGuard } from "@/components/AuthGuard";
import { AppLayout } from "@/components/AppLayout";

interface TaskListItem {
  id: string;
  title: string;
  status: string;
  repository_id: string;
  updated_at: string;
}

type StatusOption = {
  label: string;
  value: string;
  color: string;
};

const statusOptions: StatusOption[] = [
  { label: "待认领", value: "pending", color: "default" },
  { label: "已认领", value: "claimed", color: "processing" },
  { label: "执行中", value: "running", color: "warning" },
  { label: "待评审", value: "review", color: "success" },
  { label: "已落盘", value: "applied", color: "magenta" }
];

const statusTag = (status: string) => {
  const option = statusOptions.find((item) => item.value === status);
  if (!option) {
    return <Tag>{status}</Tag>;
  }
  return <Tag color={option.color}>{option.label}</Tag>;
};

export default function TaskListPage() {
  const { token } = useAuth();
  const [status, setStatus] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<TaskListItem[]>([]);

  const fetchTasks = useCallback(
    async (statusFilter?: string) => {
      if (!token) return;
      setLoading(true);
      try {
        const query = statusFilter ? `?status=${statusFilter}` : "";
        const response = await apiFetchAuthed(`/tasks${query}`, token);
        const json = (await response.json()) as TaskListItem[];
        setData(json);
      } catch (error) {
        console.error(error);
        message.error("获取任务列表失败");
      } finally {
        setLoading(false);
      }
    },
    [token]
  );

  useEffect(() => {
    void fetchTasks(status);
  }, [status, fetchTasks]);

  const columns: ColumnsType<TaskListItem> = useMemo(
    () => [
      {
        title: "标题",
        dataIndex: "title",
        render: (text, record) => <Link href={`/tasks/${record.id}`}>{text}</Link>
      },
      {
        title: "状态",
        dataIndex: "status",
        render: (value: string) => statusTag(value)
      },
      {
        title: "仓库",
        dataIndex: "repository_id",
        render: (value: string) => <Badge color="#6366f1" text={value.slice(0, 8)} />
      },
      {
        title: "更新时间",
        dataIndex: "updated_at",
        render: (value: string) => dayjs(value).format("YYYY-MM-DD HH:mm")
      }
    ],
    []
  );

  return (
    <AuthGuard>
      <AppLayout>
        <Card>
          <Flex align="center" justify="space-between" style={{ marginBottom: 16 }} wrap>
            <Typography.Title level={4} style={{ margin: 0 }}>
              任务列表
            </Typography.Title>
            <Space>
              <Select
                allowClear
                placeholder="按状态筛选"
                style={{ width: 200 }}
                onChange={(value) => setStatus(value ?? undefined)}
                options={statusOptions.map(({ label, value }) => ({ label, value }))}
              />
              <Button type="primary" onClick={() => void fetchTasks(status)}>
                刷新
              </Button>
            </Space>
          </Flex>
          <Table
            rowKey="id"
            columns={columns}
            dataSource={data}
            loading={loading}
            pagination={{ pageSize: 10 }}
          />
        </Card>
      </AppLayout>
    </AuthGuard>
  );
}
