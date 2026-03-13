export default {
    // 通用
    common: {
        appName: 'Postium',
        search: '搜尋',
        cancel: '取消',
        confirm: '確認',
        save: '儲存',
        delete: '刪除',
        edit: '編輯',
        close: '關閉',
        loading: '載入中...',
        noData: '暫無資料',
        operations: '操作',
        refresh: '重新整理',
        filter: '篩選',
        selectAll: '全選',
    },

    // 郵件
    email: {
        compose: '寫信',
        inbox: '收件匣',
        starred: '星標郵件',
        sent: '已發送',
        drafts: '草稿',
        spam: '垃圾郵件',
        trash: '已刪除',
        archive: '封存',
        subject: '主旨',
        from: '發件人',
        to: '收件人',
        cc: '副本',
        bcc: '密件副本',
        date: '日期',
        attachments: '附件',
        searchPlaceholder: '搜尋郵件...',
        noEmails: '暫無郵件',
        refreshSuccess: '郵件清單已重新整理',
        unreadCount: '{count} 封未讀',
        recipient: '收件人',
        sender: '發件人',
        reply: '回覆',
        forward: '轉寄',
        replyAll: '全部回覆',
        sending: '發送中...',
        saveDraft: '儲存草稿',
        editorPlaceholder: '在此輸入郵件內容...',
    },

    // 編輯器
    editor: {
        bold: '粗體',
        italic: '斜體',
        underline: '底線',
        strikethrough: '刪除線',
        unorderedList: '無序列表',
        orderedList: '有序列表',
        insertLink: '插入連結',
        insertImage: '插入圖片',
        addAttachment: '新增附件',
    },

    // 導航
    nav: {
        views: '檢視',
        labels: '標籤',
        calendar: '行事曆',
        workflow: '工作流程',
        settings: '設定',
        compose: '寫信',
    },

    // 設定
    settings: {
        title: '設定',
        general: '一般',
        notifications: '通知',
        ai: 'AI',
        appearance: '外觀',
        shortcuts: '快速鍵',
        language: '語言',
        theme: '主題',
        light: '淺色',
        dark: '深色',
        system: '跟隨系統',
        saveSuccess: '設定已儲存',
    },

    // 狀態列
    statusBar: {
        connected: '已連線',
        unread: '{count} 封未讀',
    },

    // 側邊欄
    sidebar: {
        storageUsed: '已使用',
        storageTotal: 'GB',
        addAccount: '新增帳號',
        labels: {
            urgent: '緊急',
            work: '工作',
            personal: '個人',
            finance: '財務',
        },
    },

    // 通知
    notifications: {
        enabled: '啟用通知',
        sound: '音效提醒',
        desktop: '桌面通知',
    },

    // AI
    ai: {
        provider: 'AI 提供商',
        apiKey: 'API 金鑰',
        model: '模型',
        generate: '生成草稿',
        improve: '改善文筆',
        shorten: '精簡內容',
        formal: '正式化',
        summary: 'AI 摘要',
        translate: '翻譯',
        tasks: '提取任務',
        smartReply: '智慧回覆',
        // AI Chat
        chat: {
            title: 'AI 助手',
            welcome: '你好！我是 AI 助手',
            welcomeDesc: '我可以協助你處理郵件，請選擇一個快速操作或直接提問',
            thinking: 'AI 正在思考...',
            sendMessage: '發送',
            inputPlaceholder: '輸入訊息... (Ctrl+Enter 發送)',
            quickActions: {
                archiveRead: '封存所有已讀郵件',
                archiveReadPrompt: '協助我封存所有已讀郵件',
                markSpam: '標記垃圾郵件',
                markSpamPrompt: '識別並標記垃圾郵件',
                organizeWork: '整理工作郵件',
                organizeWorkPrompt: '整理所有工作相關的郵件',
                findImportant: '尋找重要郵件',
                findImportantPrompt: '尋找最近一週的重要郵件',
            },
            actions: {
                execute: '執行操作',
                emails: '封郵件',
                confirm: '確定要對 {count} 封郵件執行此操作嗎？',
                result: '操作完成: 成功 {success}, 失敗 {failed}',
                failed: 'AI 回應失敗，請重試',
                operationFailed: '操作失敗，請重試',
                unknownType: '未知操作類型',
            },
        },
    },

    // 視窗控制
    window: {
        minimize: '最小化',
        maximize: '最大化',
        restore: '還原',
        close: '關閉',
    },

    // 行事曆
    calendar: {
        event: '事件',
        newEvent: '新增事件',
        editEvent: '編輯事件',
        title: '標題',
        date: '日期',
        startTime: '開始時間',
        endTime: '結束時間',
        repeat: '重複',
        repeatEnd: '結束日期',
        color: '顏色',
        notes: '備註',
        moreEvents: '更多',
        saved: '事件已儲存',
        deleted: '事件已刪除',
        today: '今天',
        prevMonth: '上一個月',
        nextMonth: '下一個月',
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
            none: '不重複',
            daily: '每天',
            weekly: '每週',
            monthly: '每月',
            yearly: '每年',
        },
        colorOptions: {
            purple: '紫色',
            blue: '藍色',
            green: '綠色',
            orange: '橙色',
            red: '紅色',
            pink: '粉色',
        },
        placeholder: {
            title: '輸入事件標題',
            notes: '新增備註...',
        },
    },

    // 工作流程
    workflow: {
        title: '工作流程',
        nodes: '節點',
        addNode: '新增節點',
        deleteNode: '刪除節點',
        save: '儲存',
        load: '載入',
        run: '執行',
        clear: '清除',
        nodeTypes: {
            trigger: '觸發器',
            action: '動作',
            condition: '條件',
            loop: '迴圈',
        },
        config: '設定',
        properties: '屬性',
    },

    // 模態框
    modal: {
        compose: '寫郵件',
        addAccount: '新增帳號',
        event: '事件',
        aiChat: 'AI 助手',
    },

    // 訊息提示
    toast: {
        success: '操作成功',
        error: '操作失敗',
        info: '提示',
        warning: '警告',
    },
};
