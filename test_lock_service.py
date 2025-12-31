#!/usr/bin/env python3
"""
分布式锁服务集成测试脚本
测试三个核心接口：申请锁、心跳、释放锁
"""

import requests
import time
import json
from typing import Dict, Any, Optional
from dataclasses import dataclass


@dataclass
class TestConfig:
    """测试配置"""
    base_url: str = "http://127.0.0.1:8080"
    acquire_endpoint: str = "/api/lock/acquire"
    heartbeat_endpoint: str = "/api/lock/heartbeat"
    release_endpoint: str = "/api/lock/release"


class LockServiceClient:
    """锁服务客户端"""
    
    def __init__(self, config: TestConfig):
        self.config = config
        self.session = requests.Session()
        self.session.headers.update({"Content-Type": "application/json"})
    
    def acquire_lock(
        self,
        namespace: str = None,
        user_id: str = "test_user",
        user_name: str = "测试用户",
        business_id: str = "test_business",
        timeout: int = 60
    ) -> Dict[str, Any]:
        """申请锁"""
        url = f"{self.config.base_url}{self.config.acquire_endpoint}"
        data = {
            "user_id": user_id,
            "user_name": user_name,
            "business_id": business_id,
            "timeout": timeout
        }
        if namespace is not None:
            data["namespace"] = namespace
            
        response = self.session.post(url, json=data)
        return response.json()
    
    def heartbeat(self, lock_id: str) -> Dict[str, Any]:
        """心跳"""
        url = f"{self.config.base_url}{self.config.heartbeat_endpoint}"
        data = {"lock_id": lock_id}
        response = self.session.post(url, json=data)
        return response.json()
    
    def release_lock(self, lock_id: str) -> Dict[str, Any]:
        """释放锁"""
        url = f"{self.config.base_url}{self.config.release_endpoint}"
        data = {"lock_id": lock_id}
        response = self.session.post(url, json=data)
        return response.json()


class TestRunner:
    """测试运行器"""
    
    def __init__(self, client: LockServiceClient):
        self.client = client
        self.passed = 0
        self.failed = 0
    
    def assert_response(self, response: Dict[str, Any], expected_success: bool, test_name: str):
        """断言响应"""
        if response.get("success") == expected_success:
            print(f"✅ {test_name}: PASSED")
            self.passed += 1
        else:
            print(f"❌ {test_name}: FAILED")
            print(f"   Expected success={expected_success}, got {response}")
            self.failed += 1
    
    def test_1_basic_acquire_and_release(self):
        """测试1：基本的申请锁和释放锁"""
        print("\n=== 测试1：基本的申请锁和释放锁 ===")
        
        # 申请锁
        response = self.client.acquire_lock(business_id="test_1")
        self.assert_response(response, True, "申请锁")
        
        if response.get("success"):
            lock_id = response["data"]["lock_id"]
            print(f"   获取到 lock_id: {lock_id}")
            
            # 释放锁
            response = self.client.release_lock(lock_id)
            self.assert_response(response, True, "释放锁")
    
    def test_2_duplicate_acquire(self):
        """测试2：不同用户重复申请同一个锁（应该失败）"""
        print("\n=== 测试2：不同用户重复申请同一个锁 ===")
        
        # 第一次申请
        response1 = self.client.acquire_lock(
            user_id="user_a",
            user_name="用户A",
            business_id="test_2"
        )
        self.assert_response(response1, True, "用户A申请锁")
        
        # 不同用户申请同一个锁（应该失败）
        response2 = self.client.acquire_lock(
            user_id="user_b",
            user_name="用户B",
            business_id="test_2"
        )
        self.assert_response(response2, False, "用户B申请锁（预期失败）")
        
        # 清理：释放第一个锁
        if response1.get("success"):
            lock_id = response1["data"]["lock_id"]
            self.client.release_lock(lock_id)
            print("   已清理锁")
    
    def test_3_heartbeat(self):
        """测试3：心跳续期"""
        print("\n=== 测试3：心跳续期 ===")
        
        # 申请锁
        response = self.client.acquire_lock(business_id="test_3", timeout=10)
        self.assert_response(response, True, "申请锁")
        
        if response.get("success"):
            lock_id = response["data"]["lock_id"]
            
            # 第一次心跳
            response = self.client.heartbeat(lock_id)
            self.assert_response(response, True, "第一次心跳")
            
            # 等待2秒后再次心跳
            time.sleep(2)
            response = self.client.heartbeat(lock_id)
            self.assert_response(response, True, "第二次心跳")
            
            # 清理
            self.client.release_lock(lock_id)
            print("   已清理锁")
    
    def test_4_lock_timeout(self):
        """测试4：锁超时自动释放"""
        print("\n=== 测试4：锁超时自动释放 ===")
        
        # 申请一个短超时的锁
        response = self.client.acquire_lock(
            user_name="用户A",
            business_id="test_4",
            timeout=3
        )
        self.assert_response(response, True, "申请短超时锁")
        
        if response.get("success"):
            lock_id = response["data"]["lock_id"]
            print(f"   等待4秒让锁过期...")
            time.sleep(4)
            
            # 尝试再次申请同一个锁（应该成功，因为之前的锁已过期）
            response = self.client.acquire_lock(
                user_name="用户B",
                business_id="test_4"
            )
            self.assert_response(response, True, "申请已过期的锁（预期成功）")
            
            # 清理
            if response.get("success"):
                new_lock_id = response["data"]["lock_id"]
                self.client.release_lock(new_lock_id)
                print("   已清理锁")
    
    def test_5_release_invalid_lock(self):
        """测试5：释放不存在的锁"""
        print("\n=== 测试5：释放不存在的锁 ===")
        
        # 尝试释放一个不存在的锁
        response = self.client.release_lock("invalid-lock-id-12345")
        self.assert_response(response, False, "释放不存在的锁（预期失败）")
    
    def test_6_heartbeat_invalid_lock(self):
        """测试6：给不存在的锁发送心跳"""
        print("\n=== 测试6：给不存在的锁发送心跳 ===")
        
        # 给不存在的锁发送心跳
        response = self.client.heartbeat("invalid-lock-id-12345")
        self.assert_response(response, False, "不存在的锁心跳（预期失败）")
    
    def test_7_namespace_isolation(self):
        """测试7：命名空间隔离"""
        print("\n=== 测试7：命名空间隔离 ===")
        
        # 在不同命名空间申请相同 business_id 的锁
        response1 = self.client.acquire_lock(
            namespace="namespace_a",
            user_name="用户A",
            business_id="test_7"
        )
        self.assert_response(response1, True, "命名空间A申请锁")
        
        response2 = self.client.acquire_lock(
            namespace="namespace_b",
            user_name="用户B",
            business_id="test_7"
        )
        self.assert_response(response2, True, "命名空间B申请锁（预期成功，不同命名空间）")
        
        # 清理
        if response1.get("success"):
            self.client.release_lock(response1["data"]["lock_id"])
        if response2.get("success"):
            self.client.release_lock(response2["data"]["lock_id"])
        print("   已清理锁")
    
    def test_8_default_namespace(self):
        """测试8：默认命名空间"""
        print("\n=== 测试8：默认命名空间 ===")
        
        # 不指定 namespace（使用默认值）
        response = self.client.acquire_lock(business_id="test_8")
        self.assert_response(response, True, "使用默认命名空间申请锁")
        
        if response.get("success"):
            lock_id = response["data"]["lock_id"]
            self.client.release_lock(lock_id)
            print("   已清理锁")
    
    def test_9_heartbeat_after_release(self):
        """测试9：释放后心跳应该失败"""
        print("\n=== 测试9：释放后心跳应该失败 ===")
        
        # 申请锁
        response = self.client.acquire_lock(business_id="test_9")
        self.assert_response(response, True, "申请锁")
        
        if response.get("success"):
            lock_id = response["data"]["lock_id"]
            
            # 释放锁
            response = self.client.release_lock(lock_id)
            self.assert_response(response, True, "释放锁")
            
            # 尝试心跳（应该失败）
            response = self.client.heartbeat(lock_id)
            self.assert_response(response, False, "释放后心跳（预期失败）")
    
    def test_10_concurrent_locks(self):
        """测试10：多个不同业务的并发锁"""
        print("\n=== 测试10：多个不同业务的并发锁 ===")
        
        lock_ids = []
        
        # 申请多个不同业务的锁
        for i in range(5):
            response = self.client.acquire_lock(
                user_name=f"用户{i}",
                business_id=f"test_10_business_{i}"
            )
            self.assert_response(response, True, f"申请锁 {i+1}/5")
            
            if response.get("success"):
                lock_ids.append(response["data"]["lock_id"])
        
        # 释放所有锁
        for i, lock_id in enumerate(lock_ids):
            response = self.client.release_lock(lock_id)
            if response.get("success"):
                print(f"   已释放锁 {i+1}/{len(lock_ids)}")
    
    def test_11_reentrant_lock(self):
        """测试11：可重入锁（同一用户重复申请）"""
        print("\n=== 测试11：可重入锁（同一用户重复申请） ===")
        
        # 第一次申请锁
        response1 = self.client.acquire_lock(
            user_id="user_reentrant",
            user_name="可重入用户",
            business_id="test_11",
            timeout=60
        )
        self.assert_response(response1, True, "第一次申请锁")
        
        if response1.get("success"):
            lock_id_1 = response1["data"]["lock_id"]
            print(f"   第一次获取的 lock_id: {lock_id_1}")
            
            # 同一用户再次申请（应该成功，返回相同的lock_id）
            response2 = self.client.acquire_lock(
                user_id="user_reentrant",
                user_name="可重入用户",
                business_id="test_11",
                timeout=60
            )
            self.assert_response(response2, True, "同一用户第二次申请锁（预期成功）")
            
            if response2.get("success"):
                lock_id_2 = response2["data"]["lock_id"]
                print(f"   第二次获取的 lock_id: {lock_id_2}")
                
                # 验证两次返回的lock_id相同
                if lock_id_1 == lock_id_2:
                    print("   ✅ 验证通过：两次返回相同的lock_id")
                    self.passed += 1
                else:
                    print(f"   ❌ 验证失败：两次返回不同的lock_id ({lock_id_1} != {lock_id_2})")
                    self.failed += 1
                
                # 第三次申请，验证仍然返回相同的lock_id
                response3 = self.client.acquire_lock(
                    user_id="user_reentrant",
                    user_name="可重入用户",
                    business_id="test_11",
                    timeout=60
                )
                self.assert_response(response3, True, "同一用户第三次申请锁（预期成功）")
                
                if response3.get("success"):
                    lock_id_3 = response3["data"]["lock_id"]
                    if lock_id_1 == lock_id_3:
                        print("   ✅ 验证通过：第三次仍返回相同的lock_id")
                    else:
                        print(f"   ❌ 验证失败：第三次返回不同的lock_id")
                
                # 清理
                self.client.release_lock(lock_id_1)
                print("   已清理锁")
    
    def test_12_reentrant_lock_different_users(self):
        """测试12：可重入锁 - 不同用户不能获取"""
        print("\n=== 测试12：可重入锁 - 验证不同用户无法获取 ===")
        
        # 用户A申请锁
        response1 = self.client.acquire_lock(
            user_id="user_a",
            user_name="用户A",
            business_id="test_12"
        )
        self.assert_response(response1, True, "用户A申请锁")
        
        if response1.get("success"):
            lock_id_a = response1["data"]["lock_id"]
            
            # 用户A再次申请（应该成功）
            response2 = self.client.acquire_lock(
                user_id="user_a",
                user_name="用户A",
                business_id="test_12"
            )
            self.assert_response(response2, True, "用户A再次申请锁（预期成功）")
            
            # 用户B尝试申请（应该失败）
            response3 = self.client.acquire_lock(
                user_id="user_b",
                user_name="用户B",
                business_id="test_12"
            )
            self.assert_response(response3, False, "用户B申请锁（预期失败）")
            
            # 清理
            self.client.release_lock(lock_id_a)
            print("   已清理锁")
    
    def run_all_tests(self):
        """运行所有测试"""
        print("\n" + "="*60)
        print("开始运行分布式锁服务集成测试")
        print("="*60)
        
        # 检查服务是否可用
        try:
            response = requests.get(f"{self.client.config.base_url}/api/lock/acquire")
        except requests.exceptions.ConnectionError:
            print("❌ 无法连接到服务，请确保服务已启动在 http://127.0.0.1:8080")
            return
        
        # 运行所有测试
        test_methods = [
            self.test_1_basic_acquire_and_release,
            self.test_2_duplicate_acquire,
            self.test_3_heartbeat,
            self.test_4_lock_timeout,
            self.test_5_release_invalid_lock,
            self.test_6_heartbeat_invalid_lock,
            self.test_7_namespace_isolation,
            self.test_8_default_namespace,
            self.test_9_heartbeat_after_release,
            self.test_10_concurrent_locks,
            self.test_11_reentrant_lock,
            self.test_12_reentrant_lock_different_users,
        ]
        
        for test_method in test_methods:
            try:
                test_method()
            except Exception as e:
                print(f"❌ 测试异常: {e}")
                self.failed += 1
        
        # 输出测试结果
        print("\n" + "="*60)
        print("测试结果汇总")
        print("="*60)
        print(f"✅ 通过: {self.passed}")
        print(f"❌ 失败: {self.failed}")
        print(f"总计: {self.passed + self.failed}")
        
        if self.failed == 0:
            print("\n🎉 所有测试通过！")
        else:
            print(f"\n⚠️  有 {self.failed} 个测试失败")


def main():
    """主函数"""
    config = TestConfig()
    client = LockServiceClient(config)
    runner = TestRunner(client)
    runner.run_all_tests()


if __name__ == "__main__":
    main()
