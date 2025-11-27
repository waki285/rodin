mod search;
pub use search::SearchPage;

use leptos::prelude::*;

use crate::frontmatter::FrontMatter;

#[component]
pub fn BlogPage(
    client_ip: String,
    html_content: String,
    meta: FrontMatter,
    current_path: String,
) -> impl IntoView {
    let article_title = meta
        .title
        .clone()
        .unwrap_or_else(|| "すずねーう".to_string());
    let subtitle_view = meta.subtitle.clone().unwrap_or_default();
    view! {
        <div class="min-h-screen bg-surface text-ink">
            <HeaderBar
                title="すずねーう".to_string()
                subtitle=format!("{client_ip}")
                current_path=current_path.clone()
            />
            <main class="mx-auto max-w-3xl p-6 prose dark:prose-invert space-y-3">
                <div class="space-y-1 mb-[0.888889em]">
                    <h1 class="text-3xl font-bold mb-0">{article_title}</h1>
                    <ShowSubtitle text=subtitle_view />
                </div>
                <MetaRow
                    published=meta.published_at.clone()
                    updated=meta.updated_at.clone()
                    reading_minutes=meta.reading_minutes
                />
                <ShowTags tags=meta.tags.clone() />
                <article inner_html=html_content></article>
            </main>
        </div>
    }
}

#[component]
pub fn TopPage(client_ip: String, home_html: String, current_path: String) -> impl IntoView {
    view! {
        <div class="relative min-h-screen text-white overflow-hidden">
            <div class="absolute inset-0">
                <picture>
                    <source
                        type="image/avif"
                        srcset="/assets/images/urumashi/urumashi-1280.avif 1280w, /assets/images/urumashi/urumashi-1920.avif 1920w, /assets/images/urumashi/urumashi-2560.avif 2560w"
                        sizes="100vw"
                    />
                    <source
                        type="image/webp"
                        srcset="/assets/images/urumashi/urumashi-1280.webp 1280w, /assets/images/urumashi/urumashi-1920.webp 1920w, /assets/images/urumashi/urumashi-2560.webp 2560w"
                        sizes="100vw"
                    />
                    <img
                        src="/assets/images/urumashi/urumashi-1280.jpg"
                        srcset="/assets/images/urumashi/urumashi-1280.jpg 1280w, /assets/images/urumashi/urumashi-1920.jpg 1920w, /assets/images/urumashi/urumashi-2560.jpg 2560w"
                        sizes="100vw"
                        width="2560"
                        height="1920"
                        alt=""
                        class="h-screen w-screen object-cover"
                        loading="eager"
                        decoding="async"
                        fetchpriority="high"
                    />
                </picture>
                <div class="absolute inset-0 bg-black/50"></div>
            </div>

            <div class="relative z-10 min-h-screen flex flex-col">
                <div class="absolute inset-x-0 top-0">
                    <HeaderBar
                        title="すずねーう".to_string()
                        subtitle=format!("{client_ip}")
                        current_path=current_path.clone()
                    />
                </div>
                <div class="flex-1 flex items-center justify-center px-6 min-h-screen">
                    <div class="bg-white/90 text-slate-900 rounded-2xl shadow-2xl px-8 py-10 max-w-md w-full backdrop-blur">
                        <div class="flex justify-center mb-6">
                            <div class="h-20 w-20 rounded-full flex items-center justify-center shadow-lg">
                                <img src="/assets/images/suzuneu.webp" alt="icon" class="h-20 w-20 object-cover rounded-full" />
                            </div>
                        </div>
                        <div class="text-center space-y-2">
                            <div class="text-4xl font-black tracking-tight text-slate-900">"すずねーう"</div>
                            <div class="text-lg text-slate-600">"自称プログラマー"</div>
                        </div>
                        <div class="flex justify-center gap-4 mt-5">
                            <SocialIcon kind="X" href="https://x.com/suzuneu_discord" class="icon-x" />
                            <SocialIcon kind="Twitter" href="https://x.com/suzuneu_discord" class="icon-twitter hidden" />
                            <SocialIcon kind="GitHub" href="https://github.com/waki285" class="" />
                            <SocialIcon kind="Discord" href="https://discord.com/users/717028469992587315" class="" />
                        </div>
                        <div class="mt-7 flex justify-center">
                            <a
                                href="/profile"
                                class="inline-flex items-center justify-center rounded-lg bg-slate-900 text-white px-5 py-2.5 text-sm font-semibold shadow-lg hover:bg-slate-800 transition-colors"
                            >
                                "プロフィール"
                            </a>
                        </div>
                    </div>
                </div>

                <main class="w-full bg-[var(--color-surface)] text-[var(--color-ink)]">
                    <div class="mx-auto max-w-3xl px-6 py-12 space-y-8">
                        <article class="prose dark:prose-invert" inner_html=home_html></article>
                    </div>
                </main>
            </div>
        </div>
    }
}

#[component]
fn SocialIcon(kind: &'static str, href: &'static str, class: &'static str) -> impl IntoView {
    let (path, view_box, fill) = match kind {
        "X" => (
            "M18.244 3.515h3.308l-7.227 7.83 8.502 9.633H16.89l-5.295-6.116-6.06 6.116H2.227l7.73-7.81L1.727 3.515H7.11l4.79 5.545zm-1.16 16.323h1.833L7.07 5.99H5.104z",
            "0 0 24 24",
            "currentColor",
        ),
        "Twitter" => (
            "M23.954 4.569c-.885.389-1.83.654-2.825.775 1.014-.611 1.794-1.574 2.163-2.723-.949.555-2.005.959-3.127 1.184-.897-.959-2.178-1.559-3.594-1.559-2.717 0-4.92 2.203-4.92 4.917 0 .39.045.765.127 1.124C7.691 8.094 4.066 6.13 1.64 3.161c-.427.722-.666 1.561-.666 2.475 0 1.71.87 3.213 2.188 4.096-.807-.026-1.566-.248-2.228-.616v.061c0 2.385 1.693 4.374 3.946 4.827-.413.111-.849.171-1.296.171-.314 0-.615-.03-.916-.086.631 1.953 2.445 3.377 4.604 3.417-1.68 1.319-3.809 2.105-6.102 2.105-.39 0-.779-.023-1.17-.067C2.179 19.29 4.768 20 7.548 20c9.142 0 14.307-7.721 13.995-14.646a9.936 9.936 0 0 0 .959-2.357z",
            "0 0 24 24",
            "#1DA1F2",
        ),
        "GitHub" => (
            "M12 .5C5.648.5.5 5.682.5 12.07c0 5.126 3.438 9.472 8.207 11.011.6.113.82-.265.82-.59 0-.292-.012-1.26-.017-2.287-3.338.73-4.042-1.63-4.042-1.63-.546-1.4-1.333-1.773-1.333-1.773-1.09-.757.083-.742.083-.742 1.205.085 1.84 1.25 1.84 1.25 1.07 1.87 2.807 1.33 3.49 1.017.107-.787.418-1.33.762-1.636-2.665-.31-5.467-1.355-5.467-6.028 0-1.332.468-2.422 1.236-3.276-.124-.31-.536-1.557.117-3.247 0 0 1.008-.327 3.3 1.252a11.347 11.347 0 0 1 3.004-.41c1.02.005 2.047.14 3.004.41 2.29-1.579 3.296-1.252 3.296-1.252.655 1.69.243 2.937.12 3.247.77.854 1.234 1.944 1.234 3.276 0 4.686-2.807 5.714-5.48 6.017.43.377.814 1.124.814 2.263 0 1.635-.015 2.954-.015 3.354 0 .328.217.71.826.59C20.066 21.536 23.5 17.19 23.5 12.07 23.5 5.682 18.352.5 12 .5Z",
            "0 0 24 24",
            "currentColor",
        ),
        "Discord" => (
            "M216.856339,16.5966031 C200.285002,8.84328665 182.566144,3.2084988 164.041564,0 C161.766523,4.11318106 159.108624,9.64549908 157.276099,14.0464379 C137.583995,11.0849896 118.072967,11.0849896 98.7430163,14.0464379 C96.9108417,9.64549908 94.1925838,4.11318106 91.8971895,0 C73.3526068,3.2084988 55.6133949,8.86399117 39.0420583,16.6376612 C5.61752293,67.146514 -3.4433191,116.400813 1.08711069,164.955721 C23.2560196,181.510915 44.7403634,191.567697 65.8621325,198.148576 C71.0772151,190.971126 75.7283628,183.341335 79.7352139,175.300261 C72.104019,172.400575 64.7949724,168.822202 57.8887866,164.667963 C59.7209612,163.310589 61.5131304,161.891452 63.2445898,160.431257 C105.36741,180.133187 151.134928,180.133187 192.754523,160.431257 C194.506336,161.891452 196.298154,163.310589 198.110326,164.667963 C191.183787,168.842556 183.854737,172.420929 176.223542,175.320965 C180.230393,183.341335 184.861538,190.991831 190.096624,198.16893 C211.238746,191.588051 232.743023,181.531619 254.911949,164.955721 C260.227747,108.668201 245.831087,59.8662432 216.856339,16.5966031 Z M85.4738752,135.09489 C72.8290281,135.09489 62.4592217,123.290155 62.4592217,108.914901 C62.4592217,94.5396472 72.607595,82.7145587 85.4738752,82.7145587 C98.3405064,82.7145587 108.709962,94.5189427 108.488529,108.914901 C108.508531,123.290155 98.3405064,135.09489 85.4738752,135.09489 Z M170.525237,135.09489 C157.88039,135.09489 147.510584,123.290155 147.510584,108.914901 C147.510584,94.5396472 157.658606,82.7145587 170.525237,82.7145587 C183.391518,82.7145587 193.761324,94.5189427 193.539891,108.914901 C193.539891,123.290155 183.391518,135.09489 170.525237,135.09489 Z",
            "0 -28.5 256 256",
            "currentColor",
        ),
        _ => (
            "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79Z",
            "0 0 24 24",
            "#5865F2",
        ),
    };

    view! {
        <a
            href=href
            class=format!("{} h-11 w-11 rounded-full bg-white/90 text-slate-900 flex items-center justify-center shadow hover:-translate-y-0.5 transition-transform", class)
            target="_blank"
            aria_label=kind
            rel="noreferrer"
        >
            <svg
                class="h-6 w-6"
                viewBox=view_box
                xmlns="http://www.w3.org/2000/svg"
                fill=fill
                stroke="none"
                aria_hidden="true"
            >
                <path d=path />
            </svg>
        </a>
    }
}

#[component]
fn HeaderBar(title: String, subtitle: String, current_path: String) -> impl IntoView {
    let title_clone = title.clone();
    let home_active = current_path == "/";
    let profile_active = current_path.starts_with("/profile");
    let search_active = current_path.starts_with("/search");
    let active_cls = "relative pb-1 after:absolute after:left-0 after:bottom-0 after:h-0.5 after:w-full after:bg-white";
    let inactive_cls = "relative pb-1 hover:text-white/80";
    view! {
        <div class="relative">
            <header id="primary-header" class="w-full h-16 header-surface text-white flex justify-center items-stretch">
                <nav class="max-w-[1200px] w-full mx-auto flex items-center justify-between px-2 relative">
                    <div class="flex items-center gap-3">
                        <a class="flex items-center gap-2" href="/" aria-label="home">
                            <img class="h-12 w-12 rounded-full" src="/assets/images/suzuneu.webp" alt="" />
                            <span class="text-2xl font-semibold tracking-wide">{title.clone()}</span>
                        </a>
                        <ShowIp subtitle=subtitle.clone() />
                    </div>
                    <ul class="hidden sm:flex items-center gap-4">
                        <li><a class=if home_active { active_cls } else { inactive_cls } href="/">"ホーム"</a></li>
                        <li><a class=if profile_active { active_cls } else { inactive_cls } href="/profile">"プロフィール"</a></li>
                        <li><a class=if search_active { active_cls } else { inactive_cls } href="/search">"検索"</a></li>
                        <li>
                            <button
                                class="theme-toggle inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-0 h-9 w-9 bg-transparent hover:text-white/80"
                                type="button"
                                aria-label="テーマ変更"
                            >
                                <ThemeIcon />
                            </button>
                        </li>
                    </ul>
                </nav>
            </header>

            <header
                id="fixed-header"
                class="pointer-events-none opacity-0 -translate-y-3 transition-all duration-300 ease-in-out fixed top-0 w-full z-40 hidden"
            >
                <nav class="max-w-[1200px] w-full mx-auto h-16 flex items-center justify-between px-2 header-surface text-white rounded-b-xl backdrop-blur">
                    <a class="flex items-center gap-2" href="/">
                        <img class="h-12 w-12 rounded-full" src="/assets/images/suzuneu.webp" alt="" />
                        <span class="text-lg font-semibold">{title_clone}</span>
                    </a>
                    <ul class="hidden sm:flex items-center gap-4">
                        <li><a class=if home_active { active_cls } else { inactive_cls } href="/">"ホーム"</a></li>
                        <li><a class=if profile_active { active_cls } else { inactive_cls } href="/profile">"プロフィール"</a></li>
                        <li><a class=if search_active { active_cls } else { inactive_cls } href="/search">"検索"</a></li>
                        <li>
                            <button
                                class="theme-toggle inline-flex items-center justify-center h-9 w-9 rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-0 bg-transparent hover:text-white/80"
                                type="button"
                                aria-label="テーマ変更"
                            >
                                <ThemeIcon />
                            </button>
                        </li>
                    </ul>
                </nav>
            </header>
        </div>
    }
}

#[component]
fn ShowIp(subtitle: String) -> impl IntoView {
    let (revealed, set_revealed) = signal(false);
    let subtitle_clone = subtitle.clone();
    view! {
        <button
            class="text-left text-sm text-white/80 underline underline-offset-2 hover:text-white"
            data-show-ip=subtitle_clone
            on:click=move |_| set_revealed.update(|prev| *prev = true)
        >
            {move || if revealed.get() { subtitle.clone() } else { "Click to Show IP".to_string() }}
        </button>
    }
}

#[component]
fn MetaRow(
    published: Option<String>,
    updated: Option<String>,
    reading_minutes: Option<u32>,
) -> impl IntoView {
    let pub_text = published.unwrap_or_else(|| "N/A".to_string());
    let upd_text = updated.unwrap_or_else(|| pub_text.clone());
    let pub_dt = pub_text.clone();
    let upd_dt = upd_text.clone();
    let read_label = reading_minutes.map(|m| format!("読むのに約 {} 分", m));
    view! {
        <div class="text-sm text-slate-600 dark:text-slate-300 mb-1 flex gap-4">
            <span>
                "Published: "
                <time datetime={pub_dt}>{pub_text}</time>
            </span>
            <span>
                "Updated: "
                <time datetime={upd_dt}>{upd_text}</time>
            </span>
            {read_label.map(|txt| view! { <span>{txt}</span> })}
        </div>
    }
}

#[component]
fn ShowTags(tags: Vec<String>) -> impl IntoView {
    let chips = tags
        .into_iter()
        .map(|t| {
            view! {
                <span class="inline-flex items-center gap-1 px-2 py-1 rounded-full border border-slate-300 dark:border-slate-600 bg-gray-100 dark:bg-gray-800 text-sm text-slate-700 dark:text-slate-200">
                    <span class="text-xs text-slate-500 dark:text-slate-300">"#"</span>
                    <span>{t}</span>
                </span>
            }
        })
        .collect::<Vec<_>>();
    view! { <div class="mb-4 flex flex-wrap gap-2">{chips}</div> }
}

#[component]
fn ShowSubtitle(text: String) -> impl IntoView {
    if text.is_empty() {
        None::<View<_>>
    } else {
        Some(view! { <p class="not-prose text-gray-600 dark:text-gray-400 py-0">{text}</p> })
    }
}

#[component]
fn ThemeIcon() -> impl IntoView {
    view! {
        <span class="inline-flex h-5 w-5 items-center justify-center relative">
            <svg
                class="theme-icon icon-sun h-5 w-5"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.75"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                focusable="false"
            >
                <circle cx="12" cy="12" r="4.5" />
                <path d="M12 2.5v2.5" />
                <path d="M12 19v2.5" />
                <path d="m4.93 4.93 1.77 1.77" />
                <path d="m17.3 17.3 1.77 1.77" />
                <path d="M2.5 12h2.5" />
                <path d="M19 12h2.5" />
                <path d="m4.93 19.07 1.77-1.77" />
                <path d="m17.3 6.7 1.77-1.77" />
            </svg>
            <svg
                class="theme-icon icon-moon h-5 w-5"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.75"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                focusable="false"
            >
                <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79Z" />
            </svg>
        </span>
    }
}
