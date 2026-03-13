export default {
    // 通用
    common: {
        appName: 'Postium',
        search: '搜索',
        cancel: '取消',
        confirm: '确认',
        save: '保存',
        delete: '删除',
        edit: '编辑',
        close: '关闭',
        loading: '加载中...',
        noData: '暂无数据',
        operations: '操作',
        refresh: '刷新',
        filter: '过滤',
        selectAll: '全选',
    },

    // 邮件
    email: {
        compose: '写信',
        inbox: '收件箱',
        starred: '星标邮件',
        sent: '已发送',
        drafts: '草稿',
        spam: '垃圾邮件',
        trash: '已删除',
        archive: '归档',
        subject: '主题',
        from: '发件人',
        to: '收件人',
        cc: '抄送',
        bcc: '密送',
        date: '日期',
        attachments: '附件',
        searchPlaceholder: '搜索邮件...',
        noEmails: '暂无邮件',
        refreshSuccess: '邮件列表已刷新',
        unreadCount: '{count} 封未读',
        recipient: '收件人',
        sender: '发件人',
        reply: '回复',
        forward: '转发',
        replyAll: '全部回复',
        sending: '发送中...',
        saveDraft: '存为草稿',
        editorPlaceholder: '在此输入邮件内容...',
    },

    // 编辑器
    editor: {
        bold: '粗体',
        italic: '斜体',
        underline: '下划线',
        strikethrough: '删除线',
        unorderedList: '无序列表',
        orderedList: '有序列表',
        insertLink: '插入链接',
        insertImage: '插入图片',
        addAttachment: '添加附件',
    },

    // 导航
    nav: {
        views: '视图',
        labels: '标签',
        calendar: '日历',
        workflow: '工作流',
        settings: '设置',
        compose: '写信',
    },

    // 设置
    settings: {
        title: '设置',
        general: '通用',
        notifications: '通知',
        ai: 'AI',
        appearance: '外观',
        shortcuts: '快捷键',
        language: '语言',
        theme: '主题',
        light: '浅色',
        dark: '深色',
        system: '跟随系统',
        saveSuccess: '设置已保存',
    },

    // 状态栏
    statusBar: {
        connected: '已连接',
        unread: '{count} 封未读',
    },

    // 侧边栏
    sidebar: {
        storageUsed: '已用',
        storageTotal: 'GB',
        addAccount: '添加账号',
        labels: {
            urgent: '紧急',
            work: '工作',
            personal: '个人',
            finance: '财务',
        },
    },

    // 通知
    notifications: {
        enabled: '启用通知',
        sound: '声音提醒',
        desktop: '桌面通知',
    },

    // AI
    ai: {
        provider: 'AI 提供商',
        apiKey: 'API 密钥',
        model: '模型',
        generate: '生成草稿',
        improve: '改进文笔',
        shorten: '精简内容',
        formal: '正式化',
        summary: 'AI 摘要',
        translate: '翻译',
        tasks: '提取任务',
        smartReply: '智能回复',
        // AI Chat
        chat: {
            title: 'AI 助手',
            welcome: '你好！我是 AI 助手',
            welcomeDesc: '我可以帮你处理邮件，请选择一个快捷操作或直接提问',
            thinking: 'AI 正在思考...',
            sendMessage: '发送',
            inputPlaceholder: '输入消息... (Ctrl+Enter 发送)',
            quickActions: {
                archiveRead: '归档所有已读邮件',
                archiveReadPrompt: '帮我归档所有已读邮件',
                markSpam: '标记垃圾邮件',
                markSpamPrompt: '识别并标记垃圾邮件',
                organizeWork: '整理工作邮件',
                organizeWorkPrompt: '整理所有工作相关的邮件',
                findImportant: '查找重要邮件',
                findImportantPrompt: '查找最近一周的重要邮件',
            },
            actions: {
                execute: '执行操作',
                emails: '封邮件',
                confirm: '确定要对 {count} 封邮件执行此操作吗？',
                result: '操作完成: 成功 {success}, 失败 {failed}',
                failed: 'AI 响应失败，请重试',
                operationFailed: '操作失败，请重试',
                unknownType: '未知操作类型',
            },
        },
    },

    // 模态框
    modal: {
        compose: '写邮件',
        addAccount: '添加账号',
        event: '事件',
        aiChat: 'AI 助手',
    },

    // 消息提示
    toast: {
        success: '操作成功',
        error: '操作失败',
        info: '提示',
        warning: '警告',
    },

    // 窗口控制
    window: {
        minimize: '最小化',
        maximize: '最大化',
        restore: '还原',
        close: '关闭',
    },

    // 日历
    calendar: {
        event: '事件',
        newEvent: '新建事件',
        editEvent: '编辑事件',
        title: '标题',
        date: '日期',
        startTime: '开始时间',
        endTime: '结束时间',
        repeat: '重复',
        repeatEnd: '结束日期',
        color: '颜色',
        notes: '备注',
        moreEvents: '更多',
        saved: '事件已保存',
        deleted: '事件已删除',
        today: '今天',
        prevMonth: '上个月',
        nextMonth: '下个月',
        weekDays: {
            sun: '日',
            mon: '一',
            tue: '二',
            wed: '三',
            thu: '四',
            fri: '五',
            sat: '六',
        },
        monthFormat: '{year}年{month}月',
        repeatOptions: {
            none: '不重复',
            daily: '每天',
            weekly: '每周',
            monthly: '每月',
            yearly: '每年',
        },
        colorOptions: {
            purple: '紫色',
            blue: '蓝色',
            green: '绿色',
            orange: '橙色',
            red: '红色',
            pink: '粉色',
        },
        placeholder: {
            title: '输入事件标题',
            notes: '添加备注...',
        },
    },

    // 工作流
    workflow: {
        title: '工作流',
        nodes: '节点',
        addNode: '添加节点',
        deleteNode: '删除节点',
        save: '保存',
        load: '加载',
        run: '运行',
        clear: '清空',
        nodeTypes: {
            trigger: '触发器',
            action: '动作',
            condition: '条件',
            loop: '循环',
        },
        config: '配置',
        properties: '属性',
    },
};
