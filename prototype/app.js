/**
 * Postium - AI-Powered Email Client
 * Main Application Script
 */

// ============================================
// Application State
// ============================================
const state = {
    currentFolder: 'inbox',
    currentEmail: null,
    emails: [],
    theme: 'dark',
    currentAccount: 'me@postium.com',
    accounts: [],
    calendar: {
        currentDate: new Date(),
        selectedDate: new Date(),
        events: [],
        editingEventId: null
    }
};

// ============================================
// Mock Email Data
// ============================================
const mockEmails = [
    {
        id: 1,
        sender: 'OpenAI Team',
        senderEmail: 'updates@openai.com',
        recipient: 'me@postium.com',
        subject: 'Introducing GPT-4 Turbo - Our Most Advanced Model',
        preview: 'We are excited to announce GPT-4 Turbo, featuring 128K context, improved accuracy, and vision capabilities...',
        body: `<p>Dear Postium User,</p>
               <p>We are thrilled to introduce <strong>GPT-4 Turbo</strong>, our most capable and efficient AI model to date.</p>
               <p>This new model brings significant improvements:</p>
               <ul>
                   <li><strong>Enhanced Speed:</strong> 3x faster response times</li>
                   <li><strong>Larger Context:</strong> 128K context window</li>
                   <li><strong>Vision Capabilities:</strong> Understand and analyze images</li>
                   <li><strong>Updated Knowledge:</strong> Information through April 2024</li>
               </ul>
               <p>Start exploring GPT-4 Turbo today in your Postium settings.</p>
               <p>Best regards,<br>The OpenAI Team</p>`,
        date: new Date(Date.now() - 1000 * 60 * 30),
        unread: true,
        starred: false,
        labels: ['work'],
        folder: 'inbox',
        attachments: []
    },
    {
        id: 2,
        sender: 'Anthropic',
        senderEmail: 'news@anthropic.com',
        recipient: 'me@postium.com',
        subject: 'Claude 3 Opus - Setting New Standards in AI Safety',
        preview: 'Discover how Claude 3 Opus is pushing the boundaries of AI safety and alignment...',
        body: `<p>Hello,</p>
               <p>We're proud to announce significant advancements in AI safety with Claude 3 Opus.</p>
               <p><strong>Key Highlights:</strong></p>
               <ul>
                   <li>Industry-leading safety protocols</li>
                   <li>Reduced hallucinations by 40%</li>
                   <li>Enhanced reasoning capabilities</li>
               </ul>
               <p>Learn more about our safety research.</p>
               <p>Warmly,<br>Anthropic Team</p>`,
        date: new Date(Date.now() - 1000 * 60 * 60 * 2),
        unread: true,
        starred: true,
        labels: ['work'],
        folder: 'inbox',
        attachments: []
    },
    {
        id: 3,
        sender: 'Project Manager',
        senderEmail: 'pm@company.com',
        recipient: 'me@postium.com',
        subject: 'Weekly Project Update - Q1 Goals Review',
        preview: 'Please review the attached Q1 progress report and prepare for Monday\'s meeting...',
        body: `<p>Hi Team,</p>
               <p>Please find attached our Q1 progress report.</p>
               <p><strong>Key points for Monday's meeting:</strong></p>
               <ol>
                   <li>Sprint velocity increased by 15%</li>
                   <li>3 major features completed</li>
                   <li>Customer satisfaction up 20%</li>
               </ol>
               <p><strong>Meeting:</strong> Monday, March 11th at 10:00 AM in Conference Room A</p>
               <p>Best,<br>Project Manager</p>`,
        date: new Date(Date.now() - 1000 * 60 * 60 * 24),
        unread: false,
        starred: false,
        labels: ['work', 'urgent'],
        folder: 'inbox',
        attachments: [
            { name: 'Q1_Report.pdf', size: '2.4 MB' },
            { name: 'Meeting_Agenda.docx', size: '156 KB' }
        ]
    },
    {
        id: 4,
        sender: 'GitHub',
        senderEmail: 'noreply@github.com',
        recipient: 'me@postium.com',
        subject: '[postium] New pull request: AI Integration Module',
        preview: 'A new pull request has been submitted to your repository...',
        body: `<p>A new pull request has been opened:</p>
               <p><strong>PR #42: AI Integration Module</strong></p>
               <p>Changes: +1,234 / -567 across 12 files</p>`,
        date: new Date(Date.now() - 1000 * 60 * 60 * 48),
        unread: false,
        starred: false,
        labels: [],
        folder: 'inbox',
        attachments: []
    },
    {
        id: 5,
        sender: 'Tech Weekly',
        senderEmail: 'digest@techweekly.com',
        recipient: 'me@postium.com',
        subject: 'This Week in Tech: AI Breakthroughs and More',
        preview: 'Your weekly digest of the most important tech news...',
        body: `<p>Your Weekly Tech Digest</p>
               <p><strong>Top Stories:</strong></p>
               <ul>
                   <li>AI assistants become mainstream</li>
                   <li>New programming languages for AI</li>
                   <li>Cloud computing costs decline</li>
               </ul>`,
        date: new Date(Date.now() - 1000 * 60 * 60 * 72),
        unread: true,
        starred: false,
        labels: [],
        folder: 'inbox',
        attachments: []
    },
    {
        id: 6,
        sender: 'Me',
        senderEmail: 'me@postium.com',
        recipient: 'client@business.com',
        subject: 'Re: Project Proposal - Final Review',
        preview: 'Thank you for your feedback. I have incorporated all suggested changes...',
        body: `<p>Dear Client,</p>
               <p>Thank you for your feedback on the project proposal.</p>`,
        date: new Date(Date.now() - 1000 * 60 * 60 * 4),
        unread: false,
        starred: false,
        labels: [],
        folder: 'sent',
        attachments: []
    }
];

// AI Summaries
const aiSummaries = {
    1: [
        'OpenAI released GPT-4 Turbo model',
        '3x faster with 128K context window',
        'New vision capabilities for image analysis',
        'Knowledge updated through April 2024'
    ],
    2: [
        'Claude 3 Opus advances in AI safety',
        'Hallucinations reduced by 40%',
        'Enhanced reasoning capabilities',
        'Better alignment with human values'
    ],
    3: [
        'Q1 progress report attached',
        'Sprint velocity up 15%',
        '3 major features completed early',
        'Meeting Monday 10 AM - Conference Room A'
    ]
};

// Smart Reply Suggestions
const smartReplies = [
    { type: 'Confirm', content: 'Thank you for the update. I\'ve reviewed the information and will prepare for the meeting.' },
    { type: 'Question', content: 'Thanks for sharing. Could you provide more details about the timeline?' },
    { type: 'Brief', content: 'Received. I\'ll get back to you shortly.' }
];

// ============================================
// Email Accounts Data
// ============================================
const mockAccounts = [
    {
        id: 'me@postium.com',
        name: '个人邮箱',
        email: 'me@postium.com',
        provider: 'imap',
        color: '#7C3AED',
        unreadCount: 3
    },
    {
        id: 'work@company.com',
        name: '工作邮箱',
        email: 'work@company.com',
        provider: 'outlook',
        color: '#10B981',
        unreadCount: 12
    }
];

// ============================================
// Calendar Events Data
// ============================================
const mockEvents = [
    {
        id: 1,
        title: '团队会议',
        date: new Date(new Date().setHours(0, 0, 0, 0)),
        startTime: '10:00',
        endTime: '11:30',
        repeat: 'weekly',
        color: '#7C3AED',
        notes: '每周团队进度同步会议'
    },
    {
        id: 2,
        title: '项目截止',
        date: new Date(new Date().getTime() + 2 * 24 * 60 * 60 * 1000),
        startTime: '18:00',
        endTime: '18:00',
        repeat: 'none',
        color: '#F43F5E',
        notes: '提交Q1报告'
    },
    {
        id: 3,
        title: '健身',
        date: new Date(),
        startTime: '07:00',
        endTime: '08:00',
        repeat: 'daily',
        color: '#10B981',
        notes: '晨间锻炼'
    }
];

// ============================================
// Initialization
// ============================================
document.addEventListener('DOMContentLoaded', () => {
    state.emails = [...mockEmails];
    state.calendar.events = [...mockEvents];
    state.accounts = [...mockAccounts];
    initApp();
});

// ============================================
// Settings Modal Functions
// ============================================
function openSettingsModal() {
    const modal = document.getElementById('settingsModal');
    if (modal) {
        modal.classList.add('show');
        updateThemeOptions();
    } else {
        console.error('Settings modal not found');
    }
}

function closeSettingsModal() {
    const modal = document.getElementById('settingsModal');
    if (modal) {
        modal.classList.remove('show');
    }
}

function updateThemeOptions() {
    const currentTheme = document.documentElement.getAttribute('data-theme') || 'dark';
    document.querySelectorAll('.theme-option').forEach(option => {
        if (option.dataset.theme === currentTheme) {
            option.classList.add('active');
        } else {
            option.classList.remove('active');
        }
    });
}

function initApp() {
    renderEmailList();
    renderAccountList();
    bindEvents();
    loadTheme();
    initCalendar();
    updateStatusBar();
    setInterval(updateStatusBar, 1000); // 每秒更新时间
    initSettingsOptions();
}

function initSettingsOptions() {
    // Settings menu navigation
    document.querySelectorAll('.settings-menu-item').forEach(item => {
        item.addEventListener('click', () => {
            const category = item.dataset.category;

            // Update menu active state
            document.querySelectorAll('.settings-menu-item').forEach(i => i.classList.remove('active'));
            item.classList.add('active');

            // Update panel visibility
            document.querySelectorAll('.settings-panel').forEach(panel => panel.classList.remove('active'));
            const targetPanel = document.getElementById(`panel-${category}`);
            if (targetPanel) {
                targetPanel.classList.add('active');
            }
        });
    });

    // Theme selection in settings
    document.querySelectorAll('.theme-option').forEach(option => {
        option.addEventListener('click', () => {
            const theme = option.dataset.theme;
            document.documentElement.setAttribute('data-theme', theme);
            localStorage.setItem('theme', theme);
            updateThemeOptions();
        });
    });
}

// ============================================
// Event Bindings
// ============================================
function bindEvents() {
    // Settings
    const settingsBtn = document.getElementById('settingsBtn');
    console.log('Settings button found:', settingsBtn);
    if (settingsBtn) {
        settingsBtn.addEventListener('click', (e) => {
            console.log('Settings button clicked');
            e.preventDefault();
            e.stopPropagation();
            openSettingsModal();
        });
    } else {
        console.error('Settings button not found!');
    }

    const closeSettingsBtn = document.getElementById('closeSettingsModal');
    if (closeSettingsBtn) {
        closeSettingsBtn.addEventListener('click', closeSettingsModal);
    }

    // Theme toggle
    document.getElementById('themeToggle').addEventListener('click', toggleTheme);

    // Compose
    document.getElementById('composeBtn').addEventListener('click', openCompose);
    document.getElementById('closeCompose').addEventListener('click', closeCompose);
    document.getElementById('minimizeCompose').addEventListener('click', closeCompose);
    document.getElementById('sendEmail').addEventListener('click', sendEmail);
    document.getElementById('saveDraft').addEventListener('click', saveDraft);

    // AI Compose
    document.getElementById('aiComposeToggle').addEventListener('click', toggleAICompose);
    document.getElementById('aiSendPrompt').addEventListener('click', handleAIPrompt);
    document.querySelectorAll('.ai-quick-btn').forEach(btn => {
        btn.addEventListener('click', () => handleAIQuickAction(btn.dataset.action));
    });

    // Editor toolbar
    document.querySelectorAll('.toolbar-btn[data-command]').forEach(btn => {
        btn.addEventListener('click', () => document.execCommand(btn.dataset.command, false, null));
    });

    // Navigation
    document.querySelectorAll('.nav-item[data-folder]').forEach(item => {
        item.addEventListener('click', (e) => {
            e.preventDefault();
            selectFolder(item.dataset.folder);
        });
    });

    // Email actions
    document.getElementById('replyBtn').addEventListener('click', replyToEmail);
    document.getElementById('forwardBtn').addEventListener('click', forwardEmail);
    document.getElementById('starBtn').addEventListener('click', toggleStar);
    document.getElementById('deleteBtn').addEventListener('click', deleteEmail);
    document.getElementById('prevEmail').addEventListener('click', () => navigateEmail(-1));
    document.getElementById('nextEmail').addEventListener('click', () => navigateEmail(1));

    // AI Actions
    document.querySelectorAll('.ai-action-btn').forEach(btn => {
        btn.addEventListener('click', () => handleAIAction(btn.dataset.action));
    });

    // Smart Reply Modal
    document.getElementById('closeSmartReply').addEventListener('click', closeSmartReply);

    // Search
    document.getElementById('searchInput').addEventListener('input', (e) => searchEmails(e.target.value));

    // Calendar
    document.getElementById('prevMonth').addEventListener('click', () => navigateMonth(-1));
    document.getElementById('nextMonth').addEventListener('click', () => navigateMonth(1));
    document.getElementById('addEventBtn').addEventListener('click', openEventModal);
    document.getElementById('closeEventModal').addEventListener('click', closeEventModal);
    document.getElementById('cancelEventBtn').addEventListener('click', closeEventModal);
    document.getElementById('saveEventBtn').addEventListener('click', saveEvent);
    document.getElementById('deleteEventBtn').addEventListener('click', deleteEvent);
    document.getElementById('eventRepeat').addEventListener('change', (e) => {
        document.getElementById('repeatEndDateGroup').style.display = e.target.value !== 'none' ? 'flex' : 'none';
    });

    // Account Management
    document.getElementById('accountSelector').addEventListener('click', toggleAccountDropdown);
    document.getElementById('closeAccountModal').addEventListener('click', closeAccountModal);
    document.getElementById('cancelAccountBtn').addEventListener('click', closeAccountModal);
    document.getElementById('saveAccountBtn').addEventListener('click', saveAccount);

    // Close dropdown when clicking outside
    document.addEventListener('click', (e) => {
        if (!e.target.closest('.custom-select')) {
            document.getElementById('accountSelector').classList.remove('open');
        }
    });

    // Keyboard shortcuts
    document.addEventListener('keydown', handleKeyboard);
}

// ============================================
// Theme
// ============================================
function toggleTheme() {
    state.theme = state.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = state.theme;
    localStorage.setItem('postium_theme', state.theme);
    showToast(`${state.theme === 'dark' ? '深色' : '浅色'}模式已启用`, 'info');
}

function loadTheme() {
    const saved = localStorage.getItem('postium_theme');
    if (saved) {
        state.theme = saved;
        document.documentElement.dataset.theme = saved;
    }
}

// ============================================
// Email List
// ============================================
function renderEmailList() {
    const list = document.getElementById('emailList');
    const emails = getFilteredEmails();

    if (emails.length === 0) {
        list.innerHTML = `
            <div style="padding: 40px; text-align: center; color: var(--text-muted);">
                <p>此文件夹中没有邮件</p>
            </div>
        `;
        return;
    }

    list.innerHTML = emails.map(email => `
        <div class="email-item ${email.unread ? 'unread' : ''} ${state.currentEmail?.id === email.id ? 'active' : ''}"
             data-id="${email.id}" onclick="selectEmail(${email.id})">
            <div class="email-item-header">
                <span class="email-sender">${email.sender}</span>
                <span class="email-time">${formatDate(email.date)}</span>
            </div>
            <div class="email-subject">${email.subject}</div>
            <div class="email-preview">${email.preview}</div>
            ${email.labels.length > 0 || email.starred ? `
                <div class="email-labels">
                    ${email.starred ? '<span class="email-label" style="background: rgba(245, 158, 11, 0.15); color: #F59E0B;">Starred</span>' : ''}
                    ${email.labels.map(l => `<span class="email-label ${l}">${capitalize(l)}</span>`).join('')}
                </div>
            ` : ''}
        </div>
    `).join('');
}

function getFilteredEmails() {
    let emails = state.emails;

    if (state.currentFolder === 'starred') {
        emails = emails.filter(e => e.starred);
    } else if (state.currentFolder !== 'all') {
        emails = emails.filter(e => e.folder === state.currentFolder);
    }

    if (state.searchQuery) {
        const q = state.searchQuery.toLowerCase();
        emails = emails.filter(e =>
            e.subject.toLowerCase().includes(q) ||
            e.sender.toLowerCase().includes(q) ||
            e.preview.toLowerCase().includes(q)
        );
    }

    return emails.sort((a, b) => b.date - a.date);
}

function selectFolder(folder) {
    state.currentFolder = folder;
    state.currentEmail = null;

    document.querySelectorAll('.nav-item').forEach(item => item.classList.remove('active'));
    document.querySelector(`.nav-item[data-folder="${folder}"]`)?.classList.add('active');

    // Handle calendar view
    if (folder === 'schedule') {
        document.getElementById('listPanel').style.display = 'none';
        document.getElementById('detailPanel').style.display = 'none';
        document.getElementById('workflowFullView').style.display = 'none';
        document.getElementById('calendarFullView').style.display = 'block';
        renderCalendar();
        return;
    }

    // Handle workflow view
    if (folder === 'workflow') {
        document.getElementById('listPanel').style.display = 'none';
        document.getElementById('detailPanel').style.display = 'none';
        document.getElementById('calendarFullView').style.display = 'none';
        document.getElementById('workflowFullView').style.display = 'flex';
        initWorkflow();
        return;
    }

    // Reset to email view
    document.getElementById('listPanel').style.display = 'flex';
    document.getElementById('detailPanel').style.display = 'flex';
    document.getElementById('calendarFullView').style.display = 'none';

    renderEmailList();
    showEmptyState();
}

function searchEmails(query) {
    state.searchQuery = query;
    renderEmailList();
}

// ============================================
// Email Detail
// ============================================
function selectEmail(id) {
    const email = state.emails.find(e => e.id === id);
    if (!email) return;

    state.currentEmail = email;
    email.unread = false;

    renderEmailList();
    showEmailDetail(email);
}

function showEmailDetail(email) {
    document.getElementById('emptyState').style.display = 'none';
    document.getElementById('emailDetail').style.display = 'flex';

    // Subject
    document.getElementById('detailSubject').textContent = email.subject;

    // Sender
    document.getElementById('senderAvatar').textContent = getInitials(email.sender);
    document.getElementById('senderName').textContent = email.sender;
    document.getElementById('senderEmail').textContent = `<${email.senderEmail}>`;
    document.getElementById('recipientEmail').textContent = email.recipient;
    document.getElementById('emailDate').textContent = formatFullDate(email.date);

    // Star
    const starBtn = document.getElementById('starBtn');
    starBtn.style.color = email.starred ? '#F59E0B' : '';

    // AI Summary
    renderAISummary(email);

    // Body
    document.getElementById('detailBody').innerHTML = email.body;

    // Attachments
    const attachSection = document.getElementById('attachmentsSection');
    const attachList = document.getElementById('attachmentsList');

    if (email.attachments.length > 0) {
        attachSection.style.display = 'block';
        attachList.innerHTML = email.attachments.map(att => `
            <div class="attachment-item">
                <div class="attachment-icon">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                        <polyline points="14 2 14 8 20 8"/>
                    </svg>
                </div>
                <div class="attachment-info">
                    <span class="attachment-name">${att.name}</span>
                    <span class="attachment-size">${att.size}</span>
                </div>
            </div>
        `).join('');
    } else {
        attachSection.style.display = 'none';
    }
}

function showEmptyState() {
    document.getElementById('emptyState').style.display = 'flex';
    document.getElementById('emailDetail').style.display = 'none';
}

function renderAISummary(email) {
    const content = document.getElementById('aiSummaryContent');
    const summary = aiSummaries[email.id];

    if (summary) {
        content.innerHTML = `<ul>${summary.map(p => `<li>${p}</li>`).join('')}</ul>`;
    } else {
        content.innerHTML = '<p>此邮件暂无 AI 摘要。</p>';
    }
}

// ============================================
// Email Actions
// ============================================
function replyToEmail() {
    if (!state.currentEmail) return;
    openCompose();
    document.getElementById('composeTo').value = state.currentEmail.senderEmail;
    document.getElementById('composeSubject').value = `Re: ${state.currentEmail.subject}`;
}

function forwardEmail() {
    if (!state.currentEmail) return;
    openCompose();
    document.getElementById('composeSubject').value = `Fwd: ${state.currentEmail.subject}`;
    document.getElementById('composeEditor').innerHTML = `
        <br><p>---------- Forwarded message ----------</p>
        <p>From: ${state.currentEmail.sender}</p>
        <p>Date: ${formatFullDate(state.currentEmail.date)}</p>
        <p>Subject: ${state.currentEmail.subject}</p>
        <br>${state.currentEmail.body}
    `;
}

function toggleStar() {
    if (!state.currentEmail) return;
    state.currentEmail.starred = !state.currentEmail.starred;
    document.getElementById('starBtn').style.color = state.currentEmail.starred ? '#F59E0B' : '';
    renderEmailList();
    showToast(state.currentEmail.starred ? '已添加到星标邮件' : '已从星标邮件中移除', 'success');
}

function deleteEmail() {
    if (!state.currentEmail) return;
    state.currentEmail.folder = 'trash';
    state.currentEmail = null;
    showEmptyState();
    renderEmailList();
    showToast('已移至已删除', 'success');
}

function navigateEmail(direction) {
    const emails = getFilteredEmails();
    const idx = emails.findIndex(e => e.id === state.currentEmail?.id);
    if (idx === -1) return;

    const newIdx = idx + direction;
    if (newIdx >= 0 && newIdx < emails.length) {
        selectEmail(emails[newIdx].id);
    }
}

// ============================================
// Compose
// ============================================
function openCompose() {
    document.getElementById('composeModal').classList.add('show');
    document.getElementById('composeTo').value = '';
    document.getElementById('composeSubject').value = '';
    document.getElementById('composeEditor').innerHTML = '';
}

function closeCompose() {
    document.getElementById('composeModal').classList.remove('show');
}

function sendEmail() {
    const to = document.getElementById('composeTo').value;
    if (!to) {
        showToast('请输入收件人', 'error');
        return;
    }

    showToast('正在发送邮件...', 'info');
    setTimeout(() => {
        const newEmail = {
            id: Date.now(),
            sender: 'Me',
            senderEmail: 'me@postium.com',
            recipient: to,
            subject: document.getElementById('composeSubject').value || '(No subject)',
            preview: document.getElementById('composeEditor').innerText.slice(0, 100),
            body: document.getElementById('composeEditor').innerHTML,
            date: new Date(),
            unread: false,
            starred: false,
            labels: [],
            folder: 'sent',
            attachments: []
        };
        state.emails.unshift(newEmail);
        closeCompose();
        showToast('邮件发送成功', 'success');
    }, 1500);
}

function saveDraft() {
    showToast('草稿已保存', 'success');
}

function toggleAICompose() {
    const header = document.getElementById('aiComposeToggle');
    const body = document.getElementById('aiComposeBody');
    header.classList.toggle('collapsed');
    body.classList.toggle('collapsed');
}

function handleAIPrompt() {
    const prompt = document.getElementById('aiPrompt').value;
    if (!prompt) return;

    showToast('AI 正在生成...', 'info');
    setTimeout(() => {
        document.getElementById('composeEditor').innerHTML = `
            <p>Hello,</p>
            <p>${prompt}</p>
            <p>This is an AI-generated draft based on your request.</p>
            <p>Best regards</p>
        `;
        showToast('草稿已生成', 'success');
    }, 1500);
}

function handleAIQuickAction(action) {
    const actions = {
        generate: '正在生成草稿...',
        improve: '正在改进写作...',
        shorten: '正在精简内容...',
        formal: '正在调整正式程度...'
    };
    showToast(actions[action] || '正在处理...', 'info');
    setTimeout(() => showToast('完成', 'success'), 1000);
}

// ============================================
// AI Actions
// ============================================
function handleAIAction(action) {
    if (!state.currentEmail) return;

    switch (action) {
        case 'smart-reply':
            openSmartReply();
            break;
        case 'summarize':
            showToast('摘要已生成', 'success');
            break;
        case 'translate':
            showToast('翻译就绪', 'success');
            break;
        case 'extract-tasks':
            showToast('任务已提取', 'success');
            break;
    }
}

function openSmartReply() {
    const container = document.getElementById('replySuggestions');
    container.innerHTML = smartReplies.map(r => `
        <div class="reply-suggestion" onclick="useReply('${r.content.replace(/'/g, "\\'")}')">
            <div class="reply-suggestion-type">${r.type}</div>
            <div class="reply-suggestion-content">${r.content}</div>
        </div>
    `).join('');
    document.getElementById('smartReplyModal').classList.add('show');
}

function closeSmartReply() {
    document.getElementById('smartReplyModal').classList.remove('show');
}

function useReply(content) {
    closeSmartReply();
    replyToEmail();
    document.getElementById('composeEditor').innerHTML = `<p>${content}</p>`;
}

// ============================================
// Keyboard
// ============================================
function handleKeyboard(e) {
    // Escape
    if (e.key === 'Escape') {
        closeCompose();
        closeSmartReply();
    }

    // Cmd/Ctrl + K - Search
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        document.getElementById('searchInput').focus();
    }

    // Cmd/Ctrl + N - Compose
    if ((e.metaKey || e.ctrlKey) && e.key === 'n') {
        e.preventDefault();
        openCompose();
    }
}

// ============================================
// Toast
// ============================================
function showToast(message, type = 'info') {
    const container = document.getElementById('toastContainer');
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;

    const icons = {
        success: '<svg class="toast-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>',
        error: '<svg class="toast-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>',
        info: '<svg class="toast-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>'
    };

    toast.innerHTML = `${icons[type]}<span class="toast-message">${message}</span>`;
    container.appendChild(toast);

    setTimeout(() => {
        toast.style.opacity = '0';
        toast.style.transform = 'translateY(20px)';
        setTimeout(() => toast.remove(), 300);
    }, 3000);
}

// ============================================
// Utilities
// ============================================
function formatDate(date) {
    const now = new Date();
    const diff = now - date;
    const mins = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (mins < 1) return '刚刚';
    if (mins < 60) return `${mins}分钟前`;
    if (hours < 24) return `${hours}小时前`;
    if (days < 7) return `${days}天前`;
    return date.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' });
}

function formatFullDate(date) {
    return date.toLocaleDateString('zh-CN', {
        weekday: 'short',
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
    });
}

function getInitials(name) {
    return name.split(' ').map(w => w[0]).join('').toUpperCase().slice(0, 2);
}

function capitalize(str) {
    return str.charAt(0).toUpperCase() + str.slice(1);
}

// ============================================
// Calendar Functions
// ============================================
function initCalendar() {
    renderCalendar();
}

function renderCalendar() {
    const grid = document.getElementById('calendarGrid');
    const title = document.getElementById('calendarTitle');
    const year = state.calendar.currentDate.getFullYear();
    const month = state.calendar.currentDate.getMonth();

    title.textContent = `${year}年${month + 1}月`;

    const firstDay = new Date(year, month, 1);
    const lastDay = new Date(year, month + 1, 0);
    const startDay = firstDay.getDay();
    const totalDays = lastDay.getDate();

    const prevMonthLastDay = new Date(year, month, 0).getDate();

    let html = '';

    // Previous month days
    for (let i = startDay - 1; i >= 0; i--) {
        const day = prevMonthLastDay - i;
        const date = new Date(year, month - 1, day);
        html += renderCalendarDay(date, true);
    }

    // Current month days
    for (let day = 1; day <= totalDays; day++) {
        const date = new Date(year, month, day);
        html += renderCalendarDay(date, false);
    }

    // Next month days
    const remainingCells = 42 - (startDay + totalDays);
    for (let day = 1; day <= remainingCells; day++) {
        const date = new Date(year, month + 1, day);
        html += renderCalendarDay(date, true);
    }

    grid.innerHTML = html;
}

function renderCalendarDay(date, isOtherMonth) {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const dateNormalized = new Date(date);
    dateNormalized.setHours(0, 0, 0, 0);

    const isToday = dateNormalized.getTime() === today.getTime();
    const dayEvents = getEventsForDate(date);

    let eventsHtml = dayEvents.map(event => `
        <div class="calendar-event" style="background: ${event.color};" onclick="event.stopPropagation(); editEvent(${event.id})">
            ${event.title}
        </div>
    `).join('');

    return `
        <div class="calendar-day ${isOtherMonth ? 'other-month' : ''} ${isToday ? 'today' : ''}" onclick="openEventModalForDate('${date.toISOString()}')">
            <div class="calendar-day-number">${date.getDate()}</div>
            <div class="calendar-events">${eventsHtml}</div>
        </div>
    `;
}

function getEventsForDate(date) {
    const dateNormalized = new Date(date);
    dateNormalized.setHours(0, 0, 0, 0);

    return state.calendar.events.filter(event => {
        const eventDate = new Date(event.date);
        eventDate.setHours(0, 0, 0, 0);

        // Check if event matches this date considering repeats
        if (event.repeat === 'none') {
            return eventDate.getTime() === dateNormalized.getTime();
        } else if (event.repeat === 'daily') {
            return eventDate <= dateNormalized;
        } else if (event.repeat === 'weekly') {
            const dayDiff = Math.floor((dateNormalized - eventDate) / (7 * 24 * 60 * 60 * 1000));
            return eventDate <= dateNormalized && eventDate.getDay() === dateNormalized.getDay();
        } else if (event.repeat === 'monthly') {
            return eventDate <= dateNormalized && eventDate.getDate() === dateNormalized.getDate();
        } else if (event.repeat === 'yearly') {
            return eventDate <= dateNormalized &&
                   eventDate.getMonth() === dateNormalized.getMonth() &&
                   eventDate.getDate() === dateNormalized.getDate();
        }
        return false;
    });
}

function navigateMonth(delta) {
    const newDate = new Date(state.calendar.currentDate);
    newDate.setMonth(newDate.getMonth() + delta);
    state.calendar.currentDate = newDate;
    renderCalendar();
}

function openEventModalForDate(dateStr) {
    const date = new Date(dateStr);
    state.calendar.selectedDate = date;
    state.calendar.editingEventId = null;

    document.getElementById('eventModalTitle').textContent = '添加事务';
    document.getElementById('deleteEventBtn').style.display = 'none';
    document.getElementById('eventTitle').value = '';
    document.getElementById('eventDate').value = date.toISOString().split('T')[0];
    document.getElementById('eventStartTime').value = '09:00';
    document.getElementById('eventEndTime').value = '10:00';
    document.getElementById('eventRepeat').value = 'none';
    document.getElementById('eventRepeatEnd').value = '';
    document.getElementById('repeatEndDateGroup').style.display = 'none';
    document.getElementById('eventNotes').value = '';
    document.querySelector('input[name="eventColor"][value="#7C3AED"]').checked = true;

    document.getElementById('eventModal').classList.add('show');
}

function openEventModal() {
    const today = new Date();
    state.calendar.selectedDate = today;
    state.calendar.editingEventId = null;

    document.getElementById('eventModalTitle').textContent = '添加事务';
    document.getElementById('deleteEventBtn').style.display = 'none';
    document.getElementById('eventTitle').value = '';
    document.getElementById('eventDate').value = today.toISOString().split('T')[0];
    document.getElementById('eventStartTime').value = '09:00';
    document.getElementById('eventEndTime').value = '10:00';
    document.getElementById('eventRepeat').value = 'none';
    document.getElementById('eventRepeatEnd').value = '';
    document.getElementById('repeatEndDateGroup').style.display = 'none';
    document.getElementById('eventNotes').value = '';
    document.querySelector('input[name="eventColor"][value="#7C3AED"]').checked = true;

    document.getElementById('eventModal').classList.add('show');
}

function closeEventModal() {
    document.getElementById('eventModal').classList.remove('show');
}

function editEvent(eventId) {
    const event = state.calendar.events.find(e => e.id === eventId);
    if (!event) return;

    state.calendar.editingEventId = eventId;

    document.getElementById('eventModalTitle').textContent = '编辑事务';
    document.getElementById('deleteEventBtn').style.display = 'block';
    document.getElementById('eventTitle').value = event.title;
    document.getElementById('eventDate').value = new Date(event.date).toISOString().split('T')[0];
    document.getElementById('eventStartTime').value = event.startTime || '09:00';
    document.getElementById('eventEndTime').value = event.endTime || '10:00';
    document.getElementById('eventRepeat').value = event.repeat || 'none';
    document.getElementById('eventRepeatEnd').value = event.repeatEnd || '';
    document.getElementById('repeatEndDateGroup').style.display = event.repeat !== 'none' ? 'flex' : 'none';
    document.getElementById('eventNotes').value = event.notes || '';
    document.querySelector(`input[name="eventColor"][value="${event.color}"]`).checked = true;

    document.getElementById('eventModal').classList.add('show');
}

function saveEvent() {
    const title = document.getElementById('eventTitle').value.trim();
    if (!title) {
        showToast('请输入事务标题', 'error');
        return;
    }

    const eventData = {
        id: state.calendar.editingEventId || Date.now(),
        title: title,
        date: new Date(document.getElementById('eventDate').value),
        startTime: document.getElementById('eventStartTime').value,
        endTime: document.getElementById('eventEndTime').value,
        repeat: document.getElementById('eventRepeat').value,
        repeatEnd: document.getElementById('eventRepeatEnd').value || null,
        color: document.querySelector('input[name="eventColor"]:checked').value,
        notes: document.getElementById('eventNotes').value
    };

    if (state.calendar.editingEventId) {
        const index = state.calendar.events.findIndex(e => e.id === state.calendar.editingEventId);
        if (index !== -1) {
            state.calendar.events[index] = eventData;
        }
        showToast('事务已更新', 'success');
    } else {
        state.calendar.events.push(eventData);
        showToast('事务已添加', 'success');
    }

    closeEventModal();
    renderCalendar();
}

function deleteEvent() {
    if (!state.calendar.editingEventId) return;

    state.calendar.events = state.calendar.events.filter(e => e.id !== state.calendar.editingEventId);
    showToast('事务已删除', 'success');
    closeEventModal();
    renderCalendar();
}

// ============================================
// Account Management Functions
// ============================================
function renderAccountList() {
    const selector = document.getElementById('accountSelector');
    const options = document.getElementById('accountOptions');
    const currentAccount = state.accounts.find(a => a.id === state.currentAccount);

    // Update trigger
    if (currentAccount) {
        document.getElementById('selectedAccountAvatar').style.background = `linear-gradient(135deg, ${currentAccount.color}, ${lightenColor(currentAccount.color, 20)})`;
        document.getElementById('selectedAccountAvatar').textContent = currentAccount.name.charAt(0);
        document.getElementById('selectedAccountName').textContent = currentAccount.name;
        document.getElementById('selectedAccountEmail').textContent = currentAccount.email;
    }

    // Update options
    options.innerHTML = state.accounts.map(account => `
        <div class="account-option ${account.id === state.currentAccount ? 'active' : ''}" data-account="${account.id}" onclick="selectAccount('${account.id}')">
            <div class="account-option-avatar" style="background: linear-gradient(135deg, ${account.color}, ${lightenColor(account.color, 20)});">${account.name.charAt(0)}</div>
            <div class="account-option-info">
                <span class="account-option-name">${account.name}</span>
                <span class="account-option-email">${account.email}</span>
            </div>
        </div>
    `).join('') + `
        <div class="account-option add-option" onclick="openAccountModal()">
            <div class="add-option-icon">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <line x1="12" y1="5" x2="12" y2="19"/>
                    <line x1="5" y1="12" x2="19" y2="12"/>
                </svg>
            </div>
            <div class="account-option-info">
                <span class="account-option-name" style="color: var(--primary);">添加邮箱账号</span>
            </div>
        </div>
    `;
}

function toggleAccountDropdown() {
    const selector = document.getElementById('accountSelector');
    selector.classList.toggle('open');
}

function selectAccount(accountId) {
    state.currentAccount = accountId;
    renderAccountList();
    document.getElementById('accountSelector').classList.remove('open');
    showToast('已切换到 ' + state.accounts.find(a => a.id === accountId)?.name, 'success');
}

function openAccountModal() {
    document.getElementById('accountModalTitle').textContent = '添加邮箱账号';
    document.getElementById('accountName').value = '';
    document.getElementById('accountEmail').value = '';
    document.getElementById('accountProvider').value = 'gmail';
    document.querySelector('input[name="accountColor"][value="#7C3AED"]').checked = true;
    document.getElementById('accountModal').classList.add('show');
}

function closeAccountModal() {
    document.getElementById('accountModal').classList.remove('show');
}

function saveAccount() {
    const name = document.getElementById('accountName').value.trim();
    const email = document.getElementById('accountEmail').value.trim();
    const provider = document.getElementById('accountProvider').value;
    const color = document.querySelector('input[name="accountColor"]:checked').value;

    if (!name || !email) {
        showToast('请填写完整的账号信息', 'error');
        return;
    }

    if (!email.includes('@')) {
        showToast('请输入有效的邮箱地址', 'error');
        return;
    }

    const newAccount = {
        id: email,
        name: name,
        email: email,
        provider: provider,
        color: color,
        unreadCount: 0
    };

    state.accounts.push(newAccount);
    renderAccountList();
    closeAccountModal();
    showToast(`邮箱账号 ${email} 添加成功`, 'success');
}

function lightenColor(color, percent) {
    const num = parseInt(color.replace('#', ''), 16);
    const amt = Math.round(2.55 * percent);
    const R = (num >> 16) + amt;
    const G = (num >> 8 & 0x00FF) + amt;
    const B = (num & 0x0000FF) + amt;
    return '#' + (0x1000000 + (R < 255 ? R < 1 ? 0 : R : 255) * 0x10000 + (G < 255 ? G < 1 ? 0 : G : 255) * 0x100 + (B < 255 ? B < 1 ? 0 : B : 255)).toString(16).slice(1);
}

// ============================================
// Status Bar
// ============================================
function updateStatusBar() {
    const currentTimeElement = document.getElementById('currentTime');
    if (currentTimeElement) {
        const now = new Date();
        const timeString = now.toLocaleTimeString('zh-CN', {
            hour: '2-digit',
            minute: '2-digit'
        });
        const dateString = now.toLocaleDateString('zh-CN', {
            month: 'short',
            day: 'numeric'
        });
        currentTimeElement.textContent = `${dateString} ${timeString}`;
    }

    // 更新未读邮件数量
    const emailCountElement = document.getElementById('emailCount');
    if (emailCountElement) {
        const unreadCount = state.emails.filter(e => !e.read).length;
        emailCountElement.innerHTML = `<span>${unreadCount} 封未读邮件</span>`;
    }
}

// ============================================
// Workflow Editor
// ============================================
let workflowNodes = [];
let nodeIdCounter = 0;

function initWorkflow() {
    const canvas = document.getElementById('workflowCanvas');
    const paletteNodes = document.querySelectorAll('.palette-node');

    // Setup drag from palette
    paletteNodes.forEach(node => {
        node.addEventListener('dragstart', handlePaletteDragStart);
        node.addEventListener('dragend', handlePaletteDragEnd);
    });

    // Setup drop zone
    canvas.addEventListener('dragover', handleCanvasDragOver);
    canvas.addEventListener('drop', handleCanvasDrop);

    // Setup workflow buttons
    document.getElementById('saveWorkflowBtn').addEventListener('click', saveWorkflow);
    document.getElementById('clearWorkflowBtn').addEventListener('click', clearWorkflow);
}

function handlePaletteDragStart(e) {
    e.dataTransfer.setData('nodeType', e.target.dataset.type);
    e.dataTransfer.setData('nodeLabel', e.target.querySelector('span').textContent);
    e.target.classList.add('dragging');
}

function handlePaletteDragEnd(e) {
    e.target.classList.remove('dragging');
}

function handleCanvasDragOver(e) {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
}

function handleCanvasDrop(e) {
    e.preventDefault();
    const nodeType = e.dataTransfer.getData('nodeType');
    const nodeLabel = e.dataTransfer.getData('nodeLabel');

    if (nodeType && nodeLabel) {
        addWorkflowNode(nodeType, nodeLabel);
    }
}

function addWorkflowNode(type, label) {
    const canvas = document.getElementById('canvasGrid');
    const emptyState = document.getElementById('canvasEmptyState');

    // Hide empty state
    if (emptyState) {
        emptyState.style.display = 'none';
    }

    // Create node element
    const nodeId = `node-${++nodeIdCounter}`;
    const node = document.createElement('div');
    node.className = 'workflow-node';
    node.id = nodeId;
    node.dataset.type = type;
    node.draggable = true;

    // Get icon based on type
    const iconSvg = getNodeIcon(type);

    node.innerHTML = `
        ${iconSvg}
        <span class="workflow-node-title">${label}</span>
        <div class="workflow-node-delete" onclick="removeWorkflowNode('${nodeId}')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"/>
                <line x1="6" y1="6" x2="18" y2="18"/>
            </svg>
        </div>
    `;

    // Add drag functionality for reordering
    node.addEventListener('dragstart', handleNodeDragStart);
    node.addEventListener('dragend', handleNodeDragEnd);
    node.addEventListener('dragover', handleNodeDragOver);
    node.addEventListener('drop', handleNodeDrop);

    canvas.appendChild(node);

    // Add to state
    workflowNodes.push({
        id: nodeId,
        type: type,
        label: label
    });

    // Select the new node
    selectNode(nodeId);
}

function getNodeIcon(type) {
    const icons = {
        trigger: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polygon points="12 2 2 7 12 12 22 7 12 2"/>
            <polyline points="2 17 12 22 22 17"/>
            <polyline points="2 12 12 17 22 12"/>
        </svg>`,
        condition: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2z"/>
            <path d="M8 12h8"/>
        </svg>`,
        action: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
            <path d="M9 12h6"/>
            <path d="M12 9v6"/>
        </svg>`,
        email: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/>
            <polyline points="22 6 12 13 2 6"/>
        </svg>`,
        delay: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <polyline points="12 6 12 12 16 14"/>
        </svg>`,
        ai: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 2a2 2 0 0 1 2 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 0 1 7 7h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1v1a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-1H2a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1a7 7 0 0 1 7-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 0 1 2-2z"/>
        </svg>`
    };
    return icons[type] || icons.action;
}

function handleNodeDragStart(e) {
    e.dataTransfer.setData('nodeId', e.target.id);
    e.target.classList.add('dragging');
}

function handleNodeDragEnd(e) {
    e.target.classList.remove('dragging');
}

function handleNodeDragOver(e) {
    e.preventDefault();
    if (e.target.classList.contains('workflow-node') && e.target !== e.target.querySelector('.dragging')) {
        e.target.style.transform = 'translateY(20px)';
    }
}

function handleNodeDrop(e) {
    e.preventDefault();
    const draggedNodeId = e.dataTransfer.getData('nodeId');
    const targetNode = e.target.closest('.workflow-node');

    if (targetNode && targetNode.id !== draggedNodeId) {
        const canvas = document.getElementById('canvasGrid');
        const draggedNode = document.getElementById(draggedNodeId);

        // Insert before or after based on position
        const rect = targetNode.getBoundingClientRect();
        const midY = rect.top + rect.height / 2;

        if (e.clientY < midY) {
            canvas.insertBefore(draggedNode, targetNode);
        } else {
            canvas.insertBefore(draggedNode, targetNode.nextSibling);
        }

        // Update nodes array
        updateNodesOrder();
    }

    // Reset transform
    document.querySelectorAll('.workflow-node').forEach(node => {
        node.style.transform = '';
    });
}

function updateNodesOrder() {
    const canvas = document.getElementById('canvasGrid');
    const nodeElements = canvas.querySelectorAll('.workflow-node');
    workflowNodes = Array.from(nodeElements).map(el => ({
        id: el.id,
        type: el.dataset.type,
        label: el.querySelector('.workflow-node-title').textContent
    }));
}

function removeWorkflowNode(nodeId) {
    const node = document.getElementById(nodeId);
    if (node) {
        node.remove();
        workflowNodes = workflowNodes.filter(n => n.id !== nodeId);

        // Show empty state if no nodes
        if (workflowNodes.length === 0) {
            const emptyState = document.getElementById('canvasEmptyState');
            if (emptyState) {
                emptyState.style.display = 'block';
            }
        }
    }
}

function selectNode(nodeId) {
    document.querySelectorAll('.workflow-node').forEach(node => {
        node.classList.remove('selected');
    });
    const node = document.getElementById(nodeId);
    if (node) {
        node.classList.add('selected');
    }
}

function clearWorkflow() {
    const canvas = document.getElementById('canvasGrid');
    const nodes = canvas.querySelectorAll('.workflow-node');
    nodes.forEach(node => node.remove());
    workflowNodes = [];
    nodeIdCounter = 0;

    const emptyState = document.getElementById('canvasEmptyState');
    if (emptyState) {
        emptyState.style.display = 'block';
    }

    showToast('工作流程已清空', 'info');
}

function saveWorkflow() {
    if (workflowNodes.length === 0) {
        showToast('请先添加工作流节点', 'warning');
        return;
    }

    // Save to localStorage
    localStorage.setItem('workflowNodes', JSON.stringify(workflowNodes));
    showToast('工作流程已保存', 'success');
}

function loadWorkflow() {
    const saved = localStorage.getItem('workflowNodes');
    if (saved) {
        try {
            const nodes = JSON.parse(saved);
            nodes.forEach(node => {
                addWorkflowNode(node.type, node.label);
            });
        } catch (e) {
            console.error('Failed to load workflow:', e);
        }
    }
}
