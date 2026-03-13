export default {
    // Commun
    common: {
        appName: 'Postium',
        search: 'Rechercher',
        cancel: 'Annuler',
        confirm: 'Confirmer',
        save: 'Enregistrer',
        delete: 'Supprimer',
        edit: 'Modifier',
        close: 'Fermer',
        loading: 'Chargement...',
        noData: 'Aucune donnée',
        operations: 'Opérations',
        refresh: 'Actualiser',
        filter: 'Filtrer',
        selectAll: 'Tout sélectionner',
    },

    // Email
    email: {
        compose: 'Composer',
        inbox: 'Boîte de réception',
        starred: 'Suivis',
        sent: 'Envoyés',
        drafts: 'Brouillons',
        spam: 'Spam',
        trash: 'Corbeille',
        archive: 'Archives',
        subject: 'Sujet',
        from: 'De',
        to: 'À',
        cc: 'CC',
        bcc: 'CCI',
        date: 'Date',
        attachments: 'Pièces jointes',
        searchPlaceholder: 'Rechercher des e-mails...',
        noEmails: 'Aucun e-mail',
        refreshSuccess: 'Liste des e-mails actualisée',
        unreadCount: '{count} non lu',
        recipient: 'Destinataire',
        sender: 'Expéditeur',
        reply: 'Répondre',
        forward: 'Transférer',
        replyAll: 'Répondre à tous',
        sending: 'Envoi en cours...',
        saveDraft: 'Enregistrer le brouillon',
        editorPlaceholder: 'Entrez le contenu de l\'e-mail ici...',
    },

    // Éditeur
    editor: {
        bold: 'Gras',
        italic: 'Italique',
        underline: 'Souligné',
        strikethrough: 'Barré',
        unorderedList: 'Liste à puces',
        orderedList: 'Liste numérotée',
        insertLink: 'Insérer un lien',
        insertImage: 'Insérer une image',
        addAttachment: 'Ajouter une pièce jointe',
    },

    // Navigation
    nav: {
        views: 'Vues',
        labels: 'Étiquettes',
        calendar: 'Calendrier',
        workflow: 'Workflow',
        settings: 'Paramètres',
        compose: 'Composer',
    },

    // Paramètres
    settings: {
        title: 'Paramètres',
        general: 'Général',
        notifications: 'Notifications',
        ai: 'IA',
        appearance: 'Apparence',
        shortcuts: 'Raccourcis',
        language: 'Langue',
        theme: 'Thème',
        light: 'Clair',
        dark: 'Sombre',
        system: 'Système',
        saveSuccess: 'Paramètres enregistrés',
    },

    // Barre d'état
    statusBar: {
        connected: 'Connecté',
        unread: '{count} non lu',
    },

    // Barre latérale
    sidebar: {
        storageUsed: 'Utilisé',
        storageTotal: 'Go',
        addAccount: 'Ajouter un compte',
        labels: {
            urgent: 'Urgent',
            work: 'Travail',
            personal: 'Personnel',
            finance: 'Finance',
        },
    },

    // Notifications
    notifications: {
        enabled: 'Activer les notifications',
        sound: 'Son',
        desktop: 'Notifications de bureau',
    },

    // IA
    ai: {
        provider: 'Fournisseur IA',
        apiKey: 'Clé API',
        model: 'Modèle',
        generate: 'Générer un brouillon',
        improve: 'Améliorer la rédaction',
        shorten: 'Raccourcir',
        formal: 'Rendre formel',
        summary: 'Résumé IA',
        translate: 'Traduire',
        tasks: 'Extraire les tâches',
        smartReply: 'Réponse intelligente',
        chat: {
            title: 'Assistant IA',
            welcome: 'Bonjour! Je suis votre Assistant IA',
            welcomeDesc: 'Je peux vous aider à gérer vos e-mails, choisissez une action rapide ou posez-moi une question',
            thinking: 'L\'IA réfléchit...',
            sendMessage: 'Envoyer',
            inputPlaceholder: 'Tapez un message... (Ctrl+Enter pour envoyer)',
            quickActions: {
                archiveRead: 'Archiver tous les e-mails lus',
                archiveReadPrompt: 'Aidez-moi à archiver tous les e-mails lus',
                markSpam: 'Marquer comme spam',
                markSpamPrompt: 'Identifier et marquer les e-mails spam',
                organizeWork: 'Organiser les e-mails de travail',
                organizeWorkPrompt: 'Organiser tous les e-mails liés au travail',
                findImportant: 'Trouver des e-mails importants',
                findImportantPrompt: 'Trouver des e-mails importants de la semaine dernière',
            },
            actions: {
                execute: 'Exécuter',
                emails: 'e-mails',
                confirm: 'Exécuter cette action sur {count} e-mails?',
                result: 'Opération terminée: {success} succès, {failed} échec',
                failed: 'La réponse IA a échoué, veuillez réessayer',
                operationFailed: 'L\'opération a échoué, veuillez réessayer',
                unknownType: 'Type d\'action inconnu',
            },
        },
    },

    // Modal
    modal: {
        compose: 'Composer un e-mail',
        addAccount: 'Ajouter un compte',
        event: 'Événement',
        aiChat: 'Assistant IA',
    },

    // Toast
    toast: {
        success: 'Succès',
        error: 'Erreur',
        info: 'Info',
        warning: 'Avertissement',
    },

    // Fenêtre
    window: {
        minimize: 'Réduire',
        maximize: 'Agrandir',
        restore: 'Restaurer',
        close: 'Fermer',
    },

    // Calendrier
    calendar: {
        event: 'Événement',
        newEvent: 'Nouvel événement',
        editEvent: 'Modifier l\'événement',
        title: 'Titre',
        date: 'Date',
        startTime: 'Heure de début',
        endTime: 'Heure de fin',
        repeat: 'Répéter',
        repeatEnd: 'Date de fin',
        color: 'Couleur',
        notes: 'Notes',
        moreEvents: 'plus',
        saved: 'Événement enregistré',
        deleted: 'Événement supprimé',
        today: 'Aujourd\'hui',
        prevMonth: 'Mois précédent',
        nextMonth: 'Mois suivant',
        weekDays: {
            sun: 'Dim',
            mon: 'Lun',
            tue: 'Mar',
            wed: 'Mer',
            thu: 'Jeu',
            fri: 'Ven',
            sat: 'Sam',
        },
        monthFormat: '{month} {year}',
        repeatOptions: {
            none: 'Ne pas répéter',
            daily: 'Quotidien',
            weekly: 'Hebdomadaire',
            monthly: 'Mensuel',
            yearly: 'Annuel',
        },
        colorOptions: {
            purple: 'Violet',
            blue: 'Bleu',
            green: 'Vert',
            orange: 'Orange',
            red: 'Rouge',
            pink: 'Rose',
        },
        placeholder: {
            title: 'Entrer le titre de l\'événement',
            notes: 'Ajouter des notes...',
        },
    },

    // Workflow
    workflow: {
        title: 'Workflow',
        nodes: 'Nœuds',
        addNode: 'Ajouter un nœud',
        deleteNode: 'Supprimer le nœud',
        save: 'Enregistrer',
        load: 'Charger',
        run: 'Exécuter',
        clear: 'Effacer',
        nodeTypes: {
            trigger: 'Déclencheur',
            action: 'Action',
            condition: 'Condition',
            loop: 'Boucle',
        },
        config: 'Configuration',
        properties: 'Propriétés',
    },
};
