import type { FileItem } from "../types";

// 基座阶段的演示数据：索引引擎接入后由 Rust 查询结果替代。
export const MOCK_FILES: FileItem[] = [
  { id: 1, name: "全球 AD 升级实施方案.docx", path: "D:\\项目资料\\AD升级\\", kind: "doc", size: "2.4 MB", modified: "2024/03/15 14:30", summary: "本文档提供全球 AD 升级的整体实施方案，包含时间规划、风险控制和部署步骤。" },
  { id: 2, name: "AD 升级实施计划.xlsx", path: "D:\\项目资料\\AD升级\\", kind: "sheet", size: "1.8 MB", modified: "2024/03/12 10:18", summary: "详细列出 AD 升级各阶段任务、负责人及时间节点。" },
  { id: 3, name: "域控制器迁移清单.xlsx", path: "D:\\IT运维\\AD\\", kind: "sheet", size: "892 KB", modified: "2024/03/10 16:45", summary: "汇总需迁移的域控制器清单、IP 地址与负责人。" },
  { id: 4, name: "AD-Audit-Report.pdf", path: "D:\\安全审计\\", kind: "pdf", size: "3.1 MB", modified: "2024/03/08 09:20", summary: "AD 升级前的审计报告，包含当前环境风险与改进建议。" },
  { id: 5, name: "AD 升级风险清单.docx", path: "D:\\项目资料\\AD升级\\", kind: "doc", size: "1.2 MB", modified: "2024/03/06 11:22", summary: "梳理 AD 升级过程中可能的风险项及应对措施。" },
  { id: 6, name: "旧域控下线计划.xlsx", path: "D:\\IT运维\\AD\\", kind: "sheet", size: "1.6 MB", modified: "2024/03/01 15:37", summary: "旧域控制器下线的详细计划，包括验证步骤和回退方案。" },
  { id: 7, name: "FSMO 角色迁移记录.xlsx", path: "D:\\IT运维\\AD\\", kind: "sheet", size: "980 KB", modified: "2024/02/28 13:09", summary: "记录 AD 升级过程中 FSMO 角色的迁移情况。" },
  { id: 8, name: "DFSR 迁移检查表.xlsx", path: "D:\\IT运维\\AD\\", kind: "sheet", size: "1.1 MB", modified: "2024/02/26 17:25", summary: "DFS 复制迁移各步骤的检查与确认记录。" },
  { id: 9, name: "站点与复制拓扑.vsdx", path: "D:\\项目资料\\AD升级\\", kind: "doc", size: "2.8 MB", modified: "2024/02/22 09:14", summary: "AD 升级后的站点与复制拓扑设计图，含各站点链路和带宽要求。" },
  { id: 10, name: "业务系统 LDAP 调研表.xlsx", path: "D:\\需求调研\\", kind: "sheet", size: "1.4 MB", modified: "2024/02/20 11:03", summary: "各业务系统对 LDAP 接口的需求与现状调研。" },
  { id: 11, name: "服务账号清单.xlsx", path: "D:\\IT运维\\AD\\", kind: "sheet", size: "920 KB", modified: "2024/02/18 16:40", summary: "域内服务账号、用途、负责人与密码到期时间。" },
  { id: 12, name: "GPO 清理评估报告.docx", path: "D:\\项目资料\\AD升级\\", kind: "doc", size: "1.9 MB", modified: "2024/02/15 10:21", summary: "对现有组策略对象的清理建议与影响评估。" },
  { id: 13, name: "DC 健康检查报告.html", path: "D:\\IT运维\\AD\\", kind: "doc", size: "680 KB", modified: "2024/02/10 14:05", summary: "dcdiag 与 repadmin 输出的域控健康检查汇总。" },
  { id: 14, name: "项目周报-2026-09-21.docx", path: "D:\\项目资料\\AD升级\\", kind: "doc", size: "1.3 MB", modified: "2024/02/08 09:33", summary: "AD 升级项目第 12 周周报：进度、风险与下周计划。" },
  { id: 15, name: "项目资料", path: "D:\\", kind: "folder", size: "—", modified: "2024/03/15 14:30", summary: "项目资料根目录，含 AD 升级全部文档。" },
  { id: 16, name: "产品主视觉-浅色.png", path: "D:\\项目资料\\设计\\", kind: "image", size: "4.6 MB", modified: "2024/03/14 10:18", summary: "3840 × 2160 产品主视觉浅色版。" },
  { id: 17, name: "系统架构图.png", path: "D:\\项目资料\\设计\\", kind: "image", size: "1.2 MB", modified: "2024/03/12 16:45", summary: "Web / API / 数据层系统架构示意图。" },
  { id: 18, name: "城市延时航拍.mp4", path: "D:\\视频素材\\", kind: "video", size: "312 MB", modified: "2024/03/01 15:37", summary: "1920 × 1080 城市延时航拍素材，时长 01:24。" },
];
