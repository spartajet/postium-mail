export default {
    // 共通
    common: {
        appName: 'Postium',
        search: '検索',
        cancel: 'キャンセル',
        confirm: '確認',
        save: '保存',
        delete: '削除',
        edit: '編集',
        close: '閉じる',
        loading: '読み込み中...',
        noData: 'データがありません',
        operations: '操作',
        refresh: '更新',
        filter: 'フィルター',
        selectAll: 'すべて選択',
    },

    // メール
    email: {
        compose: 'メール作成',
        inbox: '受信トレイ',
        starred: 'スター付き',
        sent: '送信済み',
        drafts: '下書き',
        spam: 'スパム',
        trash: 'ゴミ箱',
        archive: 'アーカイブ',
        subject: '件名',
        from: '差出人',
        to: '宛先',
        cc: 'CC',
        bcc: 'BCC',
        date: '日付',
        attachments: '添付ファイル',
        searchPlaceholder: 'メールを検索...',
        noEmails: 'メールがありません',
        refreshSuccess: 'メールリストを更新しました',
        unreadCount: '{count}件の未読',
        recipient: '宛先',
        sender: '差出人',
        reply: '返信',
        forward: '転送',
        replyAll: '全員に返信',
        sending: '送信中...',
        saveDraft: '下書きを保存',
        editorPlaceholder: 'ここにメールの内容を入力...',
    },

    // エディター
    editor: {
        bold: '太字',
        italic: '斜体',
        underline: '下線',
        strikethrough: '取り消し線',
        unorderedList: '箇条書き',
        orderedList: '番号付きリスト',
        insertLink: 'リンクを挿入',
        insertImage: '画像を挿入',
        addAttachment: '添付ファイルを追加',
    },

    // ナビゲーション
    nav: {
        views: '表示',
        labels: 'ラベル',
        calendar: 'カレンダー',
        workflow: 'ワークフロー',
        settings: '設定',
        compose: 'メール作成',
    },

    // 設定
    settings: {
        title: '設定',
        general: '一般',
        notifications: '通知',
        ai: 'AI',
        appearance: '外観',
        shortcuts: 'ショートカット',
        language: '言語',
        theme: 'テーマ',
        light: 'ライト',
        dark: 'ダーク',
        system: 'システム',
        saveSuccess: '設定を保存しました',
    },

    // ステータスバー
    statusBar: {
        connected: '接続済み',
        unread: '{count}件の未読',
    },

    // サイドバー
    sidebar: {
        storageUsed: '使用中',
        storageTotal: 'GB',
        addAccount: 'アカウントを追加',
        labels: {
            urgent: '重要',
            work: '仕事',
            personal: '個人',
            finance: '財務',
        },
    },

    // 通知
    notifications: {
        enabled: '通知を有効にする',
        sound: 'サウンド',
        desktop: 'デスクトップ通知',
    },

    // AI
    ai: {
        provider: 'AIプロバイダー',
        apiKey: 'APIキー',
        model: 'モデル',
        generate: '下書きを生成',
        improve: '文章を改善',
        shorten: '短くする',
        formal: 'フォーマルにする',
        summary: 'AI要約',
        translate: '翻訳',
        tasks: 'タスクを抽出',
        smartReply: 'スマート返信',
        chat: {
            title: 'AIアシスタント',
            welcome: 'こんにちは！私はAIアシスタントです',
            welcomeDesc: 'メール管理をお手伝いします。クイックアクションを選択するか、質問してください',
            thinking: 'AIが思考中...',
            sendMessage: '送信',
            inputPlaceholder: 'メッセージを入力... (Ctrl+Enterで送信)',
            quickActions: {
                archiveRead: '既読メールをすべてアーカイブ',
                archiveReadPrompt: '既読メールをすべてアーカイブしてください',
                markSpam: 'スパムとしてマーク',
                markSpamPrompt: 'スパムメールを識別してマーク',
                organizeWork: '仕事のメールを整理',
                organizeWorkPrompt: '仕事関連のメールをすべて整理',
                findImportant: '重要なメールを検索',
                findImportantPrompt: '過去1週間の重要なメールを検索',
            },
            actions: {
                execute: '実行',
                emails: '通のメール',
                confirm: '{count}通のメールに対してこの操作を実行しますか？',
                result: '操作完了: 成功{success}、失敗{failed}',
                failed: 'AI応答が失敗しました。もう一度お試しください',
                operationFailed: '操作が失敗しました。もう一度お試しください',
                unknownType: '不明な操作タイプ',
            },
        },
    },

    // モーダル
    modal: {
        compose: 'メール作成',
        addAccount: 'アカウントを追加',
        event: 'イベント',
        aiChat: 'AIアシスタント',
    },

    // トースト
    toast: {
        success: '成功',
        error: 'エラー',
        info: '情報',
        warning: '警告',
    },

    // ウィンドウ
    window: {
        minimize: '最小化',
        maximize: '最大化',
        restore: '元に戻す',
        close: '閉じる',
    },

    // カレンダー
    calendar: {
        event: 'イベント',
        newEvent: '新しいイベント',
        editEvent: 'イベントを編集',
        title: 'タイトル',
        date: '日付',
        startTime: '開始時刻',
        endTime: '終了時刻',
        repeat: '繰り返し',
        repeatEnd: '終了日',
        color: 'カラー',
        notes: 'メモ',
        moreEvents: '件以上',
        saved: 'イベントを保存しました',
        deleted: 'イベントを削除しました',
        today: '今日',
        prevMonth: '前の月',
        nextMonth: '次の月',
        weekDays: {
            sun: '日',
            mon: '月',
            tue: '火',
            wed: '水',
            thu: '木',
            fri: '金',
            sat: '土',
        },
        monthFormat: '{year}年{month}月',
        repeatOptions: {
            none: '繰り返しなし',
            daily: '毎日',
            weekly: '毎週',
            monthly: '毎月',
            yearly: '毎年',
        },
        colorOptions: {
            purple: '紫',
            blue: '青',
            green: '緑',
            orange: 'オレンジ',
            red: '赤',
            pink: 'ピンク',
        },
        placeholder: {
            title: 'イベントタイトルを入力',
            notes: 'メモを追加...',
        },
    },

    // ワークフロー
    workflow: {
        title: 'ワークフロー',
        nodes: 'ノード',
        addNode: 'ノードを追加',
        deleteNode: 'ノードを削除',
        save: '保存',
        load: '読み込み',
        run: '実行',
        clear: 'クリア',
        nodeTypes: {
            trigger: 'トリガー',
            action: 'アクション',
            condition: '条件',
            loop: 'ループ',
        },
        config: '設定',
        properties: 'プロパティ',
    },
};
