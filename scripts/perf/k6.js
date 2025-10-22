import http from "k6/http";
import { check, sleep } from "k6";
import { Trend } from "k6/metrics";

const apiBase = __ENV.API_BASE ?? "http://127.0.0.1:8000";
const token = __ENV.ACCESS_TOKEN;
const rate = Number(__ENV.RATE ?? 5);
const duration = __ENV.DURATION ?? "2m";
const vus = Number(__ENV.VUS ?? 10);
const maxVUs = Number(__ENV.MAX_VUS ?? Math.max(vus * 2, 20));
const pause = Number(__ENV.PAUSE ?? 1);

export const options = {
  scenarios: {
    tasksLifecycle: {
      executor: "constant-arrival-rate",
      rate,
      timeUnit: "1s",
      duration,
      preAllocatedVUs: vus,
      maxVUs
    }
  },
  thresholds: {
    http_req_duration: ["p(95)<1500"],
    tasks_list_duration: ["avg<750", "p(95)<1200"],
    environments_duration: ["avg<750", "p(95)<1200"]
  }
};

const tasksListTrend = new Trend("tasks_list_duration");
const environmentsTrend = new Trend("environments_duration");

function withAuth(headers = {}) {
  if (!token) {
    return headers;
  }
  return {
    ...headers,
    Authorization: `Bearer ${token}`
  };
}

export default function () {
  const listResponse = http.get(`${apiBase}/tasks`, {
    headers: withAuth({ "Content-Type": "application/json" })
  });
  tasksListTrend.add(listResponse.timings.duration);
  check(listResponse, {
    "tasks list succeeded": (resp) => resp.status === 200
  });

  const envResponse = http.get(`${apiBase}/api/codex/environments`, {
    headers: withAuth({ "Content-Type": "application/json" })
  });
  environmentsTrend.add(envResponse.timings.duration);
  check(envResponse, {
    "environments succeeded": (resp) => resp.status === 200
  });

  sleep(pause);
}
