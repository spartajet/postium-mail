export default {
    // Allgemein
    common: {
        appName: 'Postium',
        search: 'Suchen',
        cancel: 'Abbrechen',
        confirm: 'Bestätigen',
        save: 'Speichern',
        delete: 'Löschen',
        edit: 'Bearbeiten',
        close: 'Schließen',
        loading: 'Laden...',
        noData: 'Keine Daten',
        operations: 'Operationen',
        refresh: 'Aktualisieren',
        filter: 'Filtern',
        selectAll: 'Alle auswählen',
    },

    // E-Mail
    email: {
        compose: 'Verfassen',
        inbox: 'Posteingang',
        starred: 'Markiert',
        sent: 'Gesendet',
        drafts: 'Entwürfe',
        spam: 'Spam',
        trash: 'Papierkorb',
        archive: 'Archiv',
        subject: 'Betreff',
        from: 'Von',
        to: 'An',
        cc: 'CC',
        bcc: 'BCC',
        date: 'Datum',
        attachments: 'Anhänge',
        searchPlaceholder: 'E-Mails durchsuchen...',
        noEmails: 'Keine E-Mails',
        refreshSuccess: 'E-Mail-Liste aktualisiert',
        unreadCount: '{count} ungelesen',
        recipient: 'Empfänger',
        sender: 'Absender',
        reply: 'Antworten',
        forward: 'Weiterleiten',
        replyAll: 'Alle antworten',
        sending: 'Wird gesendet...',
        saveDraft: 'Entwurf speichern',
        editorPlaceholder: 'E-Mail-Inhalt hier eingeben...',
    },

    // Editor
    editor: {
        bold: 'Fett',
        italic: 'Kursiv',
        underline: 'Unterstrichen',
        strikethrough: 'Durchgestrichen',
        unorderedList: 'Aufzählungsliste',
        orderedList: 'Nummerierte Liste',
        insertLink: 'Link einfügen',
        insertImage: 'Bild einfügen',
        addAttachment: 'Anhang hinzufügen',
    },

    // Navigation
    nav: {
        views: 'Ansichten',
        labels: 'Beschriftungen',
        calendar: 'Kalender',
        workflow: 'Workflow',
        settings: 'Einstellungen',
        compose: 'Verfassen',
    },

    // Einstellungen
    settings: {
        title: 'Einstellungen',
        general: 'Allgemein',
        notifications: 'Benachrichtigungen',
        ai: 'KI',
        appearance: 'Darstellung',
        shortcuts: 'Tastenkürzel',
        language: 'Sprache',
        theme: 'Design',
        light: 'Hell',
        dark: 'Dunkel',
        system: 'System',
        saveSuccess: 'Einstellungen gespeichert',
    },

    // Statusleiste
    statusBar: {
        connected: 'Verbunden',
        unread: '{count} ungelesen',
    },

    // Seitenleiste
    sidebar: {
        storageUsed: 'Verwendet',
        storageTotal: 'GB',
        addAccount: 'Konto hinzufügen',
        labels: {
            urgent: 'Dringend',
            work: 'Arbeit',
            personal: 'Persönlich',
            finance: 'Finanzen',
        },
    },

    // Benachrichtigungen
    notifications: {
        enabled: 'Benachrichtigungen aktivieren',
        sound: 'Ton',
        desktop: 'Desktop-Benachrichtigungen',
    },

    // KI
    ai: {
        provider: 'KI-Anbieter',
        apiKey: 'API-Schlüssel',
        model: 'Modell',
        generate: 'Entwurf generieren',
        improve: 'Text verbessern',
        shorten: 'Verkürzen',
        formal: 'Formell machen',
        summary: 'KI-Zusammenfassung',
        translate: 'Übersetzen',
        tasks: 'Aufgaben extrahieren',
        smartReply: 'Intelligente Antwort',
        chat: {
            title: 'KI-Assistent',
            welcome: 'Hallo! Ich bin Ihr KI-Assistent',
            welcomeDesc: 'Ich kann Ihnen helfen, E-Mails zu verwalten. Wählen Sie eine Schnellaktion oder stellen Sie eine Frage',
            thinking: 'KI denkt nach...',
            sendMessage: 'Senden',
            inputPlaceholder: 'Nachricht eingeben... (Ctrl+Enter zum Senden)',
            quickActions: {
                archiveRead: 'Alle gelesenen E-Mails archivieren',
                archiveReadPrompt: 'Helfen Sie mir, alle gelesenen E-Mails zu archivieren',
                markSpam: 'Spam markieren',
                markSpamPrompt: 'Spam-E-Mails identifizieren und markieren',
                organizeWork: 'Arbeits-E-Mails organisieren',
                organizeWorkPrompt: 'Alle arbeitsbezogenen E-Mails organisieren',
                findImportant: 'Wichtige E-Mails finden',
                findImportantPrompt: 'Wichtige E-Mails der letzten Woche finden',
            },
            actions: {
                execute: 'Ausführen',
                emails: 'E-Mails',
                confirm: 'Diese Aktion für {count} E-Mails ausführen?',
                result: 'Operation abgeschlossen: {success} erfolgreich, {failed} fehlgeschlagen',
                failed: 'KI-Antwort fehlgeschlagen, bitte erneut versuchen',
                operationFailed: 'Operation fehlgeschlagen, bitte erneut versuchen',
                unknownType: 'Unbekannter Aktionstyp',
            },
        },
    },

    // Modal
    modal: {
        compose: 'E-Mail verfassen',
        addAccount: 'Konto hinzufügen',
        event: 'Ereignis',
        aiChat: 'KI-Assistent',
    },

    // Toast
    toast: {
        success: 'Erfolg',
        error: 'Fehler',
        info: 'Info',
        warning: 'Warnung',
    },

    // Fenster
    window: {
        minimize: 'Minimieren',
        maximize: 'Maximieren',
        restore: 'Wiederherstellen',
        close: 'Schließen',
    },

    // Kalender
    calendar: {
        event: 'Ereignis',
        newEvent: 'Neues Ereignis',
        editEvent: 'Ereignis bearbeiten',
        title: 'Titel',
        date: 'Datum',
        startTime: 'Startzeit',
        endTime: 'Endzeit',
        repeat: 'Wiederholen',
        repeatEnd: 'Enddatum',
        color: 'Farbe',
        notes: 'Notizen',
        moreEvents: 'mehr',
        saved: 'Ereignis gespeichert',
        deleted: 'Ereignis gelöscht',
        today: 'Heute',
        prevMonth: 'Vorheriger Monat',
        nextMonth: 'Nächster Monat',
        weekDays: {
            sun: 'So',
            mon: 'Mo',
            tue: 'Di',
            wed: 'Mi',
            thu: 'Do',
            fri: 'Fr',
            sat: 'Sa',
        },
        monthFormat: '{month} {year}',
        repeatOptions: {
            none: 'Nicht wiederholen',
            daily: 'Täglich',
            weekly: 'Wöchentlich',
            monthly: 'Monatlich',
            yearly: 'Jährlich',
        },
        colorOptions: {
            purple: 'Lila',
            blue: 'Blau',
            green: 'Grün',
            orange: 'Orange',
            red: 'Rot',
            pink: 'Rosa',
        },
        placeholder: {
            title: 'Ereignistitel eingeben',
            notes: 'Notizen hinzufügen...',
        },
    },

    // Workflow
    workflow: {
        title: 'Workflow',
        nodes: 'Knoten',
        addNode: 'Knoten hinzufügen',
        deleteNode: 'Knoten löschen',
        save: 'Speichern',
        load: 'Laden',
        run: 'Ausführen',
        clear: 'Löschen',
        nodeTypes: {
            trigger: 'Auslöser',
            action: 'Aktion',
            condition: 'Bedingung',
            loop: 'Schleife',
        },
        config: 'Konfiguration',
        properties: 'Eigenschaften',
    },
};
