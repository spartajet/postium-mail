export default {
    // Common
    common: {
        appName: 'Postium',
        search: 'Search',
        cancel: 'Cancel',
        confirm: 'Confirm',
        save: 'Save',
        delete: 'Delete',
        edit: 'Edit',
        close: 'Close',
        loading: 'Loading...',
        noData: 'No data',
        operations: 'Operations',
        refresh: 'Refresh',
        filter: 'Filter',
        selectAll: 'Select All',
    },

    // Email
    email: {
        compose: 'Compose',
        inbox: 'Inbox',
        starred: 'Starred',
        sent: 'Sent',
        drafts: 'Drafts',
        spam: 'Spam',
        trash: 'Trash',
        archive: 'Archive',
        subject: 'Subject',
        from: 'From',
        to: 'To',
        cc: 'CC',
        bcc: 'BCC',
        date: 'Date',
        attachments: 'Attachments',
        searchPlaceholder: 'Search emails...',
        noEmails: 'No emails',
        refreshSuccess: 'Email list refreshed',
        unreadCount: '{count} unread',
        recipient: 'Recipient',
        sender: 'Sender',
        reply: 'Reply',
        forward: 'Forward',
        replyAll: 'Reply All',
        sending: 'Sending...',
        saveDraft: 'Save Draft',
        editorPlaceholder: 'Enter email content here...',
    },

    // Editor
    editor: {
        bold: 'Bold',
        italic: 'Italic',
        underline: 'Underline',
        strikethrough: 'Strikethrough',
        unorderedList: 'Bullet List',
        orderedList: 'Numbered List',
        insertLink: 'Insert Link',
        insertImage: 'Insert Image',
        addAttachment: 'Add Attachment',
    },

    // Navigation
    nav: {
        views: 'Views',
        labels: 'Labels',
        calendar: 'Calendar',
        workflow: 'Workflow',
        settings: 'Settings',
        compose: 'Compose',
    },

    // Settings
    settings: {
        title: 'Settings',
        general: 'General',
        notifications: 'Notifications',
        ai: 'AI',
        appearance: 'Appearance',
        shortcuts: 'Shortcuts',
        language: 'Language',
        theme: 'Theme',
        light: 'Light',
        dark: 'Dark',
        system: 'System',
        saveSuccess: 'Settings saved',
    },

    // Status Bar
    statusBar: {
        connected: 'Connected',
        unread: '{count} unread',
    },

    // Sidebar
    sidebar: {
        storageUsed: 'Used',
        storageTotal: 'GB',
        addAccount: 'Add Account',
        labels: {
            urgent: 'Urgent',
            work: 'Work',
            personal: 'Personal',
            finance: 'Finance',
        },
    },

    // Notifications
    notifications: {
        enabled: 'Enable Notifications',
        sound: 'Sound',
        desktop: 'Desktop Notifications',
    },

    // AI
    ai: {
        provider: 'AI Provider',
        apiKey: 'API Key',
        model: 'Model',
        generate: 'Generate Draft',
        improve: 'Improve Writing',
        shorten: 'Shorten',
        formal: 'Make Formal',
        summary: 'AI Summary',
        translate: 'Translate',
        tasks: 'Extract Tasks',
        smartReply: 'Smart Reply',
        chat: {
            title: 'AI Assistant',
            welcome: 'Hello! I am your AI Assistant',
            welcomeDesc: 'I can help you manage emails, choose a quick action or ask me anything',
            thinking: 'AI is thinking...',
            sendMessage: 'Send',
            inputPlaceholder: 'Type a message... (Ctrl+Enter to send)',
            quickActions: {
                archiveRead: 'Archive all read emails',
                archiveReadPrompt: 'Help me archive all read emails',
                markSpam: 'Mark spam',
                markSpamPrompt: 'Identify and mark spam emails',
                organizeWork: 'Organize work emails',
                organizeWorkPrompt: 'Organize all work-related emails',
                findImportant: 'Find important emails',
                findImportantPrompt: 'Find important emails from the past week',
            },
            actions: {
                execute: 'Execute',
                emails: 'emails',
                confirm: 'Execute this action on {count} emails?',
                result: 'Operation completed: {success} success, {failed} failed',
                failed: 'AI response failed, please try again',
                operationFailed: 'Operation failed, please try again',
                unknownType: 'Unknown action type',
            },
        },
    },

    // Modal
    modal: {
        compose: 'Compose Email',
        addAccount: 'Add Account',
        event: 'Event',
        aiChat: 'AI Assistant',
    },

    // Toast
    toast: {
        success: 'Success',
        error: 'Error',
        info: 'Info',
        warning: 'Warning',
    },

    // Window
    window: {
        minimize: 'Minimize',
        maximize: 'Maximize',
        restore: 'Restore',
        close: 'Close',
    },

    // Calendar
    calendar: {
        event: 'Event',
        newEvent: 'New Event',
        editEvent: 'Edit Event',
        title: 'Title',
        date: 'Date',
        startTime: 'Start Time',
        endTime: 'End Time',
        repeat: 'Repeat',
        repeatEnd: 'End Date',
        color: 'Color',
        notes: 'Notes',
        moreEvents: 'more',
        saved: 'Event saved',
        deleted: 'Event deleted',
        today: 'Today',
        prevMonth: 'Previous month',
        nextMonth: 'Next month',
        weekDays: {
            sun: 'Sun',
            mon: 'Mon',
            tue: 'Tue',
            wed: 'Wed',
            thu: 'Thu',
            fri: 'Fri',
            sat: 'Sat',
        },
        monthFormat: '{year} {month}',
        repeatOptions: {
            none: 'Does not repeat',
            daily: 'Daily',
            weekly: 'Weekly',
            monthly: 'Monthly',
            yearly: 'Yearly',
        },
        colorOptions: {
            purple: 'Purple',
            blue: 'Blue',
            green: 'Green',
            orange: 'Orange',
            red: 'Red',
            pink: 'Pink',
        },
        placeholder: {
            title: 'Enter event title',
            notes: 'Add notes...',
        },
    },

    // Workflow
    workflow: {
        title: 'Workflow',
        nodes: 'Nodes',
        addNode: 'Add Node',
        deleteNode: 'Delete Node',
        save: 'Save',
        load: 'Load',
        run: 'Run',
        clear: 'Clear',
        nodeTypes: {
            trigger: 'Trigger',
            action: 'Action',
            condition: 'Condition',
            loop: 'Loop',
        },
        config: 'Config',
        properties: 'Properties',
    },
};
