//! The tab's words, in English and French, as `STRINGS` of logic.js had them
//! (a count `n` picks `key_one` or `key_other`). English fills in what French
//! lacks; the reasons the hook gives an agent (`why.<rule>/<kind>`) are French
//! only, since English is what the hook wrote.

use hangar_kit_sdk::i18n::Strings;

pub const EN: &[(&str, &str)] = &[
    ("tab", "Guardrails"),
    ("heading", "Refused calls"),
    ("loading", "Reading the refusals…"),
    ("stateRefused_one", "{n} refused"),
    ("stateRefused_other", "{n} refused"),
    ("stateClear", "Nothing refused"),
    ("emptyTitle", "Nothing refused yet"),
    (
        "emptyText",
        "When an agent tries a dangerous command or reads a secret, the refusal shows here, and you can allow it once.",
    ),
    ("clear", "Clear the list"),
    (
        "clearFailed",
        "The list could not be cleared. Try again in a moment.",
    ),
    ("allow", "Allow once"),
    ("allowing", "Writing the pass…"),
    ("refusedLabel", "Refused"),
    ("letThroughLabel", "Let through once"),
    ("passActive", "Allowed once, {remaining} left"),
    (
        "passActiveHint",
        "Tell {agent} to retry this exact command.",
    ),
    (
        "passExpired",
        "The pass expired unused. A retry is refused again.",
    ),
    ("passUsed", "Let through once, {when}"),
    (
        "problem.denied",
        "This tab may not write passes. Accept data.write in Settings › Kits.",
    ),
    (
        "problem.rate_limited",
        "Too many writes in a minute. Try again in a moment.",
    ),
    ("problem.failed", "The pass could not be written: {detail}"),
    ("refusedTimes", "refused {n} times"),
    (
        "truncated",
        "Cut for display: {n} characters in all. The pass is for the whole command.",
    ),
    ("staleWarning", "The last read failed. Trying again…"),
    (
        "errors_one",
        "The guard hit a problem of its own, once: either a call went through unchecked, or a broken .hangar/guard.json was ignored. Latest:",
    ),
    (
        "errors_other",
        "The guard hit a problem of its own, {n} times: either a call went through unchecked, or a broken .hangar/guard.json was ignored. Latest:",
    ),
    ("limitsHeading", "What this does not protect against."),
    (
        "limits",
        "It reads the command line, and nothing else. It is no sandbox: a script can do anything once started, and a program the rules do not list can read any file.",
    ),
    ("deniedTitle", "This tab may not read the kit’s data"),
    (
        "deniedText",
        "Accept data.read and data.write in Settings › Kits. The hook refuses either way, but nothing can be let through from here until then.",
    ),
    ("errorTitle", "The refusals could not be read"),
    (
        "errorText",
        "The hook goes on refusing. Try again; if it keeps failing, reload the tab.",
    ),
    ("retry", "Try again"),
];

pub const FR: &[(&str, &str)] = &[
    ("tab", "Garde-fous"),
    ("heading", "Appels refusés"),
    ("loading", "Lecture des refus…"),
    ("stateRefused_one", "{n} refus"),
    ("stateRefused_other", "{n} refus"),
    ("stateClear", "Rien de refusé"),
    ("emptyTitle", "Rien de refusé pour l’instant"),
    (
        "emptyText",
        "Quand un agent tente une commande dangereuse ou lit un secret, le refus s’affiche ici, et vous pouvez l’autoriser une fois.",
    ),
    ("clear", "Vider la liste"),
    (
        "clearFailed",
        "La liste n’a pas pu être vidée. Réessayez dans un instant.",
    ),
    ("allow", "Autoriser une fois"),
    ("allowing", "Écriture du laissez-passer…"),
    ("refusedLabel", "Refusé"),
    ("letThroughLabel", "Laissé passer une fois"),
    ("passActive", "Autorisé une fois, reste {remaining}"),
    (
        "passActiveHint",
        "Dites à {agent} de réessayer cette commande exacte.",
    ),
    (
        "passExpired",
        "Le laissez-passer a expiré sans servir. Un nouvel essai sera refusé.",
    ),
    ("passUsed", "Laissé passer une fois, {when}"),
    (
        "problem.denied",
        "Cet onglet n’a pas le droit d’écrire de laissez-passer. Acceptez data.write dans Réglages › Kits.",
    ),
    (
        "problem.rate_limited",
        "Trop d’écritures en une minute. Réessayez dans un instant.",
    ),
    (
        "problem.failed",
        "Le laissez-passer n’a pas pu être écrit : {detail}",
    ),
    ("refusedTimes", "refusé {n} fois"),
    (
        "truncated",
        "Coupé à l’affichage : {n} caractères en tout. Le laissez-passer vaut pour la commande entière.",
    ),
    (
        "staleWarning",
        "La dernière lecture a échoué. Nouvel essai…",
    ),
    (
        "errors_one",
        "Le garde-fou a eu un problème de son côté, une fois : soit un appel est passé sans contrôle, soit un .hangar/guard.json cassé a été ignoré. Le dernier :",
    ),
    (
        "errors_other",
        "Le garde-fou a eu un problème de son côté, {n} fois : soit un appel est passé sans contrôle, soit un .hangar/guard.json cassé a été ignoré. Le dernier :",
    ),
    ("limitsHeading", "Ce dont il ne protège pas."),
    (
        "limits",
        "Il lit la ligne de commande, et rien d’autre. Ce n’est pas un bac à sable : un script peut tout faire une fois lancé, et un programme que les règles ne citent pas peut lire n’importe quel fichier.",
    ),
    (
        "deniedTitle",
        "Cet onglet n’a pas le droit de lire les données du kit",
    ),
    (
        "deniedText",
        "Acceptez data.read et data.write dans Réglages › Kits. Le hook refuse quand même, mais rien ne peut être autorisé d’ici tant que ce n’est pas fait.",
    ),
    ("errorTitle", "Les refus n’ont pas pu être lus"),
    (
        "errorText",
        "Le hook continue de refuser. Réessayez ; si cela persiste, rechargez l’onglet.",
    ),
    ("retry", "Réessayer"),
    (
        "why.recursive-delete/outside",
        "Il supprime {subject}, hors du projet.",
    ),
    (
        "why.recursive-delete/project",
        "Il supprime tout le projet ({subject}).",
    ),
    (
        "why.recursive-delete/protected",
        "Il supprime {subject}, l’historique git du projet.",
    ),
    (
        "why.recursive-delete/unreadable",
        "Il supprime {subject}, et la ligne de commande ne dit pas où cela mène.",
    ),
    (
        "why.force-push/default",
        "Il force un push sur {subject}, une branche par défaut : cela réécrit son historique pour tout le monde.",
    ),
    (
        "why.git-reset-hard/any",
        "git reset --hard jette le travail non commité, que git ne peut pas rendre.",
    ),
    (
        "why.git-clean/any",
        "git clean --force supprime les fichiers non suivis, que git ne peut pas rendre.",
    ),
    (
        "why.drop-database/any",
        "Il supprime une base de données entière.",
    ),
    (
        "why.download-to-shell/any",
        "Il exécute du code tout droit sorti d’un téléchargement, sans que personne ne l’ait lu.",
    ),
    (
        "why.encoded-command/any",
        "Il passe à PowerShell une commande en base64 (-EncodedCommand), que personne ne peut lire avant qu’elle ne s’exécute.",
    ),
    (
        "why.secret-file/read",
        "Il lit ou affiche {subject}, un fichier secret.",
    ),
    (
        "why.secret-file/copy",
        "Il copie {subject}, un fichier secret, là où il n’est plus traité comme tel.",
    ),
    (
        "why.secret-file/write",
        "Il écrase {subject}, un fichier secret.",
    ),
    (
        "why.guard-files/any",
        "Il modifie {subject}, un fichier du garde-fou lui-même, que seule la personne modifie.",
    ),
];

/// `fr` for a locale that starts with it, as a word (`fr`, `fr-CA`, `FR`);
/// `en` for every other, English being what the tab falls back to.
pub fn pick_lang(locale: &str) -> &'static str {
    let bytes = locale.as_bytes();
    if bytes.len() < 2 || !bytes[..2].eq_ignore_ascii_case(b"fr") {
        return "en";
    }
    // `\b`, as JavaScript means it: the next character, if any, is not one of
    // [A-Za-z0-9_]. Two ASCII letters came first, so this is a boundary.
    match locale[2..].chars().next() {
        Some(c) if c.is_ascii_alphanumeric() || c == '_' => "en",
        _ => "fr",
    }
}

/// The tab's strings, spoken in the language `locale` asks for.
pub fn for_locale(locale: &str) -> Strings {
    Strings::new(pick_lang(locale)).add("en", EN).add("fr", FR)
}

/// The reason, in the tab's language. English comes as the hook told the
/// agent: its own words, with a capital for « it » and a full stop. French is
/// ours, when we know the rule.
pub fn why_for(t: &Strings, rule: &str, kind: &str, subject: &str, hook_words: &str) -> String {
    let key = format!("why.{rule}/{kind}");
    if pick_lang(t.locale()) == "fr" {
        let words = t.tv(&key, &[("subject", subject)]);
        // `Strings` answers the key itself for a reason it does not know.
        if words != key {
            return words;
        }
    }
    // Only « it » takes a capital: `git reset --hard` is a command's name.
    let words = match hook_words.strip_prefix("it ") {
        Some(rest) => format!("It {rest}"),
        None => hook_words.to_owned(),
    };
    if words.is_empty() {
        words
    } else {
        format!("{words}.")
    }
}
