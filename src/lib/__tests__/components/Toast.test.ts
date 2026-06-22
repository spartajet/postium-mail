import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock Svelte Context API so createToastState can run outside a component
vi.mock("svelte", () => ({
	setContext: vi.fn(),
	getContext: vi.fn(),
}));

// Import after mock so svelte internals are stubbed
const { createToastState } = await import("$lib/stores/toast.svelte");

describe("ToastState store 逻辑测试", () => {
	let toastState: ReturnType<typeof createToastState>;

	beforeEach(() => {
		toastState = createToastState();
	});

	// ==================== 基础功能 ====================

	it("初始状态 toasts 为空数组", () => {
		expect(toastState.toasts).toEqual([]);
	});

	// ==================== show() 方法 ====================

	it("show() 添加一条消息到 toasts 列表", () => {
		const id = toastState.show("Hello");

		expect(toastState.toasts).toHaveLength(1);
		expect(toastState.toasts[0]!.id).toBe(id);
		expect(toastState.toasts[0]!.message).toBe("Hello");
	});

	it("show() 默认类型为 info，默认时长为 3000ms", () => {
		toastState.show("默认消息");

		expect(toastState.toasts[0]!.type).toBe("info");
		expect(toastState.toasts[0]!.duration).toBe(3000);
	});

	it("show() 可以自定义类型和时长", () => {
		toastState.show("自定义消息", "error", 5000);

		expect(toastState.toasts[0]!.type).toBe("error");
		expect(toastState.toasts[0]!.duration).toBe(5000);
	});

	it("show() 返回自增的唯一 ID", () => {
		const id1 = toastState.show("第一条");
		const id2 = toastState.show("第二条");
		const id3 = toastState.show("第三条");

		expect(id1).toBeLessThan(id2);
		expect(id2).toBeLessThan(id3);
	});

	it("show() 可以添加多条消息", () => {
		toastState.show("消息1");
		toastState.show("消息2");
		toastState.show("消息3");

		expect(toastState.toasts).toHaveLength(3);
		expect(toastState.toasts.map((t) => t.message)).toEqual([
			"消息1",
			"消息2",
			"消息3",
		]);
	});

	// ==================== 快捷方法 ====================

	it("success() 创建 success 类型消息，时长 3000ms", () => {
		toastState.success("保存成功");

		expect(toastState.toasts[0]!.type).toBe("success");
		expect(toastState.toasts[0]!.message).toBe("保存成功");
		expect(toastState.toasts[0]!.duration).toBe(3000);
	});

	it("error() 创建 error 类型消息，时长 5000ms", () => {
		toastState.error("网络错误");

		expect(toastState.toasts[0]!.type).toBe("error");
		expect(toastState.toasts[0]!.message).toBe("网络错误");
		expect(toastState.toasts[0]!.duration).toBe(5000);
	});

	it("info() 创建 info 类型消息，时长 3000ms", () => {
		toastState.info("提示信息");

		expect(toastState.toasts[0]!.type).toBe("info");
		expect(toastState.toasts[0]!.message).toBe("提示信息");
		expect(toastState.toasts[0]!.duration).toBe(3000);
	});

	it("warning() 创建 warning 类型消息，时长 4000ms", () => {
		toastState.warning("存储空间不足");

		expect(toastState.toasts[0]!.type).toBe("warning");
		expect(toastState.toasts[0]!.message).toBe("存储空间不足");
		expect(toastState.toasts[0]!.duration).toBe(4000);
	});

	// ==================== dismiss() 方法 ====================

	it("dismiss() 根据ID移除指定消息", () => {
		const id1 = toastState.show("保留");
		const id2 = toastState.show("移除");
		const id3 = toastState.show("保留");

		toastState.dismiss(id2);

		expect(toastState.toasts).toHaveLength(2);
		expect(toastState.toasts.map((t) => t.id)).toEqual([id1, id3]);
	});

	it("dismiss() 移除不存在的ID时安全忽略", () => {
		toastState.show("消息");

		expect(() => toastState.dismiss(999)).not.toThrow();
		expect(toastState.toasts).toHaveLength(1);
	});

	it("dismiss() 可以逐个移除所有消息", () => {
		const id1 = toastState.show("消息1");
		const id2 = toastState.show("消息2");
		const id3 = toastState.show("消息3");

		toastState.dismiss(id1);
		expect(toastState.toasts).toHaveLength(2);

		toastState.dismiss(id2);
		expect(toastState.toasts).toHaveLength(1);

		toastState.dismiss(id3);
		expect(toastState.toasts).toHaveLength(0);
	});

	// ==================== 组合场景 ====================

	it("show 和 dismiss 可以交叉使用", () => {
		const id1 = toastState.success("成功");
		const id2 = toastState.error("失败");

		toastState.dismiss(id1);
		expect(toastState.toasts).toHaveLength(1);
		expect(toastState.toasts[0]!.type).toBe("error");

		toastState.warning("新警告");
		expect(toastState.toasts).toHaveLength(2);

		toastState.dismiss(id2);
		expect(toastState.toasts).toHaveLength(1);
		expect(toastState.toasts[0]!.type).toBe("warning");
	});

	it("快捷方法返回的 ID 可用于 dismiss", () => {
		const id = toastState.success("可关闭消息");

		toastState.dismiss(id);
		expect(toastState.toasts).toHaveLength(0);
	});

	it("Toast 接口字段完整", () => {
		toastState.show("完整测试", "warning", 6000);

		const toast = toastState.toasts[0]!;
		expect(toast).toHaveProperty("id");
		expect(toast).toHaveProperty("type");
		expect(toast).toHaveProperty("message");
		expect(toast).toHaveProperty("duration");
		expect(typeof toast.id).toBe("number");
		expect(typeof toast.type).toBe("string");
		expect(typeof toast.message).toBe("string");
		expect(typeof toast.duration).toBe("number");
	});
});
