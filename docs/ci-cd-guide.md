# lan-link CI/CD 工作流指南

> 最后更新: 2026-06-02
> 仓库: https://github.com/123654lkj/lan-link

## 概览

6 台海外 VPS 挂载了 GitHub Actions Self-Hosted Runner，组成 CI 集群。
每次 push 到 main 分支，自动触发并行编译。

## Runner 分布

| Runner 名 | 机器 | 位置 | LL 地址 |
|-----------|------|------|---------|
| ruisu1 | 锐宿1号 | 洛杉矶 | 10.0.0.3:9876 |
| ruisu2 | 锐宿2号 | 达拉斯 | 10.0.0.4:9876 |
| lvyun1 | 绿云1号 | 洛杉矶 | 10.0.0.5:9876 |
| lvyun2 | 绿云2号 | 洛杉矶 | 10.0.0.6:9876 |
| lvyun3 | 绿云3号 | 洛杉矶 | 10.0.0.7:9876 |
| xinghuer | 星狐儿 | 阿里云成都 | 10.0.0.2:9877 |

所有 runner 标签: ll-vps

## 开发工作流

```bash
# 改代码后提交推送
git add -A
git commit -m "feat: xxx"
git push origin main

# 去 https://github.com/123654lkj/lan-link/actions 看 CI 结果
```

## 关联基础设施

### WireGuard Mesh
所有机器通过 WireGuard 组网，IP段 10.0.0.0/24
- 10.0.0.1 - 团子 (本地服务器)
- 10.0.0.2~7 - VPS

### LL (lan-linkctl) 远程管理
```bash
lan-linkctl -a 10.0.0.3:9876 uptime
lan-linkctl -a 10.0.0.4:9876 info
```
