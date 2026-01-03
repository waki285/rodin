mod search;
pub use search::SearchPage;

use leptos::prelude::*;

use crate::app::render::BlogListItem;
use crate::app::render::SITE_URL;
use crate::frontmatter::FrontMatter;
use urlencoding::encode;

#[component]
pub fn ContactPage(
    client_ip: String,
    current_path: String,
    hcaptcha_site_key: Option<String>,
) -> impl IntoView {
    let site_key = hcaptcha_site_key.unwrap_or_default();
    view! {
        <div class="blog-wrapper">
            <HeaderBar
                title=crate::constants::AUTHOR_NAME.to_string()
                subtitle=format!("{client_ip}")
                current_path=current_path
            />
            <main class="contact-container">
                <h1>"お問い合わせ"</h1>
                <p class="contact-description">
                    "ご連絡ありがとうございます。必要事項をご入力のうえ送信してください。"
                </p>
                <form id="contact-form" class="contact-form" method="post" action="/api/contact" novalidate>
                    <label for="contact-name">"名前"</label>
                    <input id="contact-name" name="name" type="text" autocomplete="name" required />

                    <label for="contact-email">"メールアドレス"</label>
                    <input id="contact-email" name="email" type="email" autocomplete="email" required />

                    <label for="contact-message">"お問い合わせ内容"</label>
                    <textarea id="contact-message" name="message" rows="6" required></textarea>

                    {if site_key.is_empty() {
                        view! { <p class="contact-warning">"hCaptcha の設定が未完了です。管理者にご連絡ください。"</p> }.into_any()
                    } else {
                        view! { <div class="h-captcha" data-sitekey=site_key></div> }.into_any()
                    }}

                    <button class="contact-submit" type="submit">"送信"</button>
                    <p id="contact-status" class="contact-status" aria-live="polite"></p>
                </form>
            </main>
        </div>
    }
}

#[component]
pub fn BlogListPage(
    client_ip: String,
    posts: Vec<BlogListItem>,
    current_page: u32,
    total_pages: u32,
) -> impl IntoView {
    view! {
        <div class="blog-wrapper">
            <HeaderBar
                title=crate::constants::AUTHOR_NAME.to_string()
                subtitle=format!("{client_ip}")
                current_path="/blog".to_string()
            />
            <main class="blog-container">
                <h1 class="blog-list-title">"ブログ記事一覧"</h1>
                <div class="posts-list">
                    {posts.into_iter().map(|post| {
                        let url = format!("/blog/{}", post.slug);
                        let published = post.published_at.clone().unwrap_or_default();
                        let updated = post.updated_at.clone().unwrap_or_default();
                        let description = post.description.clone().unwrap_or_default();
                        let tags = post.tags.clone();
                        let published_html = if !published.is_empty() {
                            format!("Published: {}", published)
                        } else {
                            String::new()
                        };
                        let updated_html = if !updated.is_empty() && updated != published {
                            format!("Updated: {}", updated)
                        } else {
                            String::new()
                        };
                        let tags_html = if !tags.is_empty() {
                            let chips: String = tags
                                .iter()
                                .map(|t| {
                                    let href = format!("/tags/{}", encode(t));
                                    format!(
                                        r#"<a class="blog-tag" href="{href}"><span class="blog-tag-hash">#</span><span>{}</span></a>"#,
                                        t
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("");
                            format!("<div class=\"flex flex-wrap gap-2 pt-1\">{}</div>", chips)
                        } else {
                            String::new()
                        };
                        view! {
                            <article class="post-card">
                                <div>
                                    <a href=url class="not-prose">{post.title}</a>
                                    <div>{updated_html}</div>
                                </div>
                                <div>{description}</div>
                                <div>{published_html}</div>
                                <div inner_html=tags_html></div>
                            </article>
                        }
                    }).collect_view()}
                </div>
                <Pagination current_page=current_page total_pages=total_pages base_url="/blog".to_string() />
            </main>
        </div>
    }
}

#[component]
fn Pagination(current_page: u32, total_pages: u32, base_url: String) -> impl IntoView {
    if total_pages <= 1 {
        return None;
    }

    let prev_page = if current_page > 1 {
        Some(current_page - 1)
    } else {
        None
    };
    let next_page = if current_page < total_pages {
        Some(current_page + 1)
    } else {
        None
    };

    let page_url = |p: u32| {
        if p == 1 {
            base_url.clone()
        } else {
            format!("{}?page={}", base_url, p)
        }
    };

    Some(view! {
        <nav class="pagination" aria-label="ページナビゲーション">
            {prev_page.map(|p| {
                let url = page_url(p);
                view! { <a href=url class="pagination-prev">"← 前のページ"</a> }
            })}
            <span class="pagination-info">
                {format!("{} / {}", current_page, total_pages)}
            </span>
            {next_page.map(|p| {
                let url = page_url(p);
                view! { <a href=url class="pagination-next">"次のページ →"</a> }
            })}
        </nav>
    })
}

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
        .unwrap_or_else(|| crate::constants::AUTHOR_NAME.to_string());
    let subtitle_view = meta.subtitle.clone().unwrap_or_default();
    let crumbs = meta.breadcrumbs.clone();
    let registry = crate::app::render::breadcrumb_registry();
    let slug_opt = current_path.strip_prefix("/blog/").map(|s| s.to_string());
    view! {
        <div class="blog-wrapper">
            <HeaderBar
                title=crate::constants::AUTHOR_NAME.to_string()
                subtitle=format!("{client_ip}")
                current_path=current_path.clone()
            />
            <main class="blog-container prose dark:prose-invert">
                {(!crumbs.is_empty()).then(|| {
                    let last = crumbs.len().saturating_sub(1);
                    view! {
                        <nav aria-label="breadcrumb" class="blog-breadcrumb not-prose">
                            <ol>
                                {crumbs.iter().enumerate().map(|(idx, key)| {
                                    let is_last = idx == last;
                                    let (label, href_opt) = registry
                                        .get(key.as_str())
                                        .map(|(n, h)| (n.to_string(), Some(*h)))
                                        .unwrap_or_else(|| (key.clone(), None));
                                    view! {
                                        <li>
                                            {if is_last {
                                                view! { <span class="blog-breadcrumb-label">{label.clone()}</span> }.into_any()
                                            } else {
                                                view! {
                                                    <a href={href_opt.unwrap_or("#")} class="blog-breadcrumb-label">{label.clone()}</a>
                                                }.into_any()
                                            }}
                                            {view! { <span class="separator">/</span> }.into_any()}
                                        </li>
                                    }
                                }).collect_view()}
                            </ol>
                        </nav>
                    }
                })}
                <div class="blog-title">
                    <h1>{article_title.clone()}</h1>
                    <ShowSubtitle text=subtitle_view />
                </div>
                <MetaRow
                    published=meta.published_at.clone()
                    updated=meta.updated_at.clone()
                    reading_minutes=meta.reading_minutes
                    slug=current_path.strip_prefix("/blog/").map(|s| s.to_string())
                />
                <div class="tags-share-row">
                    <ShowTags tags=meta.tags.clone() />
                    <ShareButtons
                        title=article_title.clone()
                        slug=current_path.strip_prefix("/blog/").map(|s| s.to_string())
                    />
                </div>
                <article inner_html=html_content></article>
                {slug_opt.as_ref().map(|slug| {
                    let url = format!("https://github.com/waki285/rodin-content/blob/main/{}.typ", slug);
                    view! {
                        <div class="edit-proposal not-prose">
                            <a class="edit-proposal-btn" href=url target="_blank" rel="noopener noreferrer">
                                <svg aria-hidden="true" viewBox="0 0 24 24" focusable="false">
                                    <path d="M12 .5C5.65.5.5 5.68.5 12.07c0 5.12 3.39 9.46 8.08 10.99.59.11.8-.26.8-.58 0-.29-.01-1.24-.02-2.25-3.29.71-3.98-1.58-3.98-1.58-.54-1.39-1.32-1.76-1.32-1.76-1.08-.76.08-.75.08-.75 1.19.09 1.82 1.22 1.82 1.22 1.06 1.87 2.78 1.33 3.45 1.02.11-.77.42-1.33.76-1.64-2.63-.3-5.39-1.32-5.39-5.87 0-1.3.47-2.37 1.23-3.21-.12-.31-.54-1.57.12-3.27 0 0 1.01-.33 3.3 1.25a11.5 11.5 0 0 1 6 0c2.29-1.58 3.29-1.25 3.29-1.25.67 1.7.25 2.96.12 3.27.77.84 1.23 1.91 1.23 3.21 0 4.56-2.77 5.56-5.41 5.85.43.37.81 1.11.81 2.24 0 1.62-.02 2.93-.02 3.33 0 .32.21.7.81.58 4.69-1.53 8.08-5.87 8.08-10.99C23.5 5.68 18.35.5 12 .5Z"/>
                                </svg>
                                <span>"GitHub で編集を提案"</span>
                            </a>
                        </div>
                    }
                })}
            </main>
        </div>
    }
}

#[component]
pub fn TagListPage(client_ip: String, tag: String, posts: Vec<BlogListItem>) -> impl IntoView {
    view! {
        <div class="blog-wrapper">
            <HeaderBar
                title=crate::constants::AUTHOR_NAME.to_string()
                subtitle=format!("{client_ip}")
                current_path=format!("/tags/{tag}")
            />
            <main class="blog-container">
                <h1 class="blog-list-title">{format!("タグ: #{tag}")}</h1>
                <div class="posts-list">
                    {posts.into_iter().map(|post| {
                        let url = format!("/blog/{}", post.slug);
                        let published = post.published_at.clone().unwrap_or_default();
                        let updated = post.updated_at.clone().unwrap_or_default();
                        let description = post.description.clone().unwrap_or_default();
                        let tags = post.tags.clone();
                        let published_html = if !published.is_empty() {
                            format!("Published: {}", published)
                        } else {
                            String::new()
                        };
                        let updated_html = if !updated.is_empty() && updated != published {
                            format!("Updated: {}", updated)
                        } else {
                            String::new()
                        };
                        let tags_html = if !tags.is_empty() {
                            let chips: String = tags
                                .iter()
                                .map(|t| {
                                    let href = format!("/tags/{}", encode(t));
                                    format!(
                                        r#"<a class="blog-tag" href="{href}"><span class="blog-tag-hash">#</span><span>{}</span></a>"#,
                                        t
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("");
                            format!("<div class=\"flex flex-wrap gap-2 pt-1\">{}</div>", chips)
                        } else {
                            String::new()
                        };
                        view! {
                            <article class="post-card">
                                <div>
                                    <a href=url class="not-prose">{post.title}</a>
                                    <div>{updated_html}</div>
                                </div>
                                <div>{description}</div>
                                <div>{published_html}</div>
                                <div inner_html=tags_html></div>
                            </article>
                        }
                    }).collect_view()}
                </div>
            </main>
        </div>
    }
}

#[component]
pub fn TopPage(client_ip: String, home_html: String, current_path: String) -> impl IntoView {
    view! {
        <div class="top-container">
            <div class="top-hero">
                <picture
                    data-deferred-bg
                    data-bg-sizes="(max-width: 640px) 320px, (max-width: 1024px) 640px, 1200px"
                >
                    <source
                        media="(max-width: 768px)"
                        type="image/avif"
                        srcset="/assets/images/urumashi/urumashi-1280.avif 1280w"
                    />
                    <source
                        type="image/avif"
                        srcset="/assets/images/urumashi/urumashi-1280-low.avif 1280w, /assets/images/urumashi/urumashi-1920-low.avif 1920w, /assets/images/urumashi/urumashi-2560-low.avif 2560w"
                        data-hi-srcset="/assets/images/urumashi/urumashi-1280.avif 1280w, /assets/images/urumashi/urumashi-1920.avif 1920w, /assets/images/urumashi/urumashi-2560.avif 2560w"
                    />
                    <source
                        media="(max-width: 768px)"
                        type="image/webp"
                        data-srcset="/assets/images/urumashi/urumashi-1280.webp 1280w"
                    />
                    <source
                        type="image/webp"
                        data-srcset="/assets/images/urumashi/urumashi-1280.webp 1280w, /assets/images/urumashi/urumashi-1920.webp 1920w, /assets/images/urumashi/urumashi-2560.webp 2560w"
                    />
                    <img
                        src="/assets/images/urumashi/urumashi-1280-low.avif"
                        srcset="/assets/images/urumashi/urumashi-1280-low.avif 1280w"
                        data-src="/assets/images/urumashi/urumashi-1280.jpg"
                        data-srcset="/assets/images/urumashi/urumashi-1280.jpg 1280w, /assets/images/urumashi/urumashi-1920.jpg 1920w, /assets/images/urumashi/urumashi-2560.jpg 2560w"
                        data-sizes="(max-width: 640px) 320px, (max-width: 1024px) 640px, 1200px"
                        width="2560"
                        height="1920"
                        alt=""
                        loading="eager"
                        decoding="async"
                        fetchpriority="high"
                    />
                </picture>
                <div class="paint"></div>
            </div>

            <div class="top-content">
                <div class="top-header">
                    <HeaderBar
                        title=crate::constants::AUTHOR_NAME.to_string()
                        subtitle=format!("{client_ip}")
                        current_path=current_path.clone()
                    />
                </div>
                <div class="top-profcard-container">
                    <div class="top-profcard">
                        <div class="top-avatar">
                            <div>
                                <img src=crate::constants::ICON_URL alt="icon" />
                            </div>
                        </div>
                        <div class="top-name">
                            <div class="top-name-title">{crate::constants::AUTHOR_NAME}</div>
                            <div class="top-name-subtitle">"自称プログラマー"</div>
                        </div>
                        <div class="top-social">
                            <SocialIcon kind="X" href=crate::constants::TWITTER_URL class="icon-x" />
                            <SocialIcon kind="Twitter" href=crate::constants::TWITTER_URL class="icon-twitter hidden" />
                            <SocialIcon kind="GitHub" href=crate::constants::GITHUB_URL class="" />
                            <SocialIcon kind="Discord" href="https://discord.com/users/717028469992587315" class="" />
                        </div>
                        <div class="top-profile-link">
                            <a
                                href="/profile"
                            >
                                "プロフィール"
                            </a>
                        </div>
                    </div>
                </div>

                <main class="top-main">
                    <div>
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
            class=format!("{} social-icon", class)
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
    let blog_active = current_path.starts_with("/blog");
    let profile_active = current_path.starts_with("/profile");
    let search_active = current_path.starts_with("/search");
    let contact_active = current_path.starts_with("/contact");
    let active_cls = "active";
    let inactive_cls = "inactive";
    view! {
        <div class="header-container">
            <header id="primary-header">
                <nav>
                    <div class="header-logo">
                        <a href="/" aria-label="home">
                            <img src=crate::constants::ICON_URL alt="" width="48" height="48" />
                            <span>{title.clone()}</span>
                        </a>
                        <ShowIp subtitle=subtitle.clone() />
                    </div>
                    <div class="header-menu">
                        <input id="nav-toggle-main" type="checkbox" />
                        <label
                            for="nav-toggle-main"
                            aria-label="メニューを開く"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
                            </svg>
                        </label>
                        <ul class="header-links">
                            <li><a data-prefetch="true" class=if home_active { active_cls } else { inactive_cls } href="/">"ホーム"</a></li>
                            <li><a data-prefetch="true" class=if blog_active { active_cls } else { inactive_cls } href="/blog">"ブログ"</a></li>
                            <li><a data-prefetch="true" class=if profile_active { active_cls } else { inactive_cls } href="/profile">"プロフィール"</a></li>
                            <li><a data-prefetch="true" class=if search_active { active_cls } else { inactive_cls } href="/search">"検索"</a></li>
                            <li><a data-prefetch="true" class=if contact_active { active_cls } else { inactive_cls } href="/contact">"お問い合わせ"</a></li>
                            <li>
                                <button
                                    class="theme-toggle"
                                    type="button"
                                    aria-label="テーマ変更"
                                >
                                    <ThemeIcon />
                                </button>
                            </li>
                        </ul>
                        <div class="mobile-menu">
                            <ul>
                                <li><a data-prefetch="true" href="/">ホーム</a></li>
                                <li><a data-prefetch="true" href="/blog">ブログ</a></li>
                                <li><a data-prefetch="true" href="/profile">プロフィール</a></li>
                                <li><a data-prefetch="true" href="/search">検索</a></li>
                                <li><a data-prefetch="true" href="/contact">お問い合わせ</a></li>
                                <li>
                                    <button
                                        class="theme-toggle"
                                        type="button"
                                        aria-label="テーマ変更"
                                    >
                                        <ThemeIcon />
                                        <span>テーマ変更</span>
                                    </button>
                                </li>
                            </ul>
                        </div>
                    </div>
                </nav>
            </header>

            <header
                id="fixed-header"
            >
                <nav>
                    <a href="/">
                        <img src=crate::constants::ICON_URL alt="" width="48" height="48" />
                        <span>{title_clone}</span>
                    </a>
                    <div class="header-menu">
                        <input id="nav-toggle-fixed" type="checkbox" />
                        <label
                            for="nav-toggle-fixed"
                            aria-label="メニューを開く"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
                            </svg>
                        </label>
                        <ul class="header-links">
                            <li><a data-prefetch="true" class=if home_active { active_cls } else { inactive_cls } href="/">"ホーム"</a></li>
                            <li><a data-prefetch="true" class=if blog_active { active_cls } else { inactive_cls } href="/blog">"ブログ"</a></li>
                            <li><a data-prefetch="true" class=if profile_active { active_cls } else { inactive_cls } href="/profile">"プロフィール"</a></li>
                            <li><a data-prefetch="true" class=if search_active { active_cls } else { inactive_cls } href="/search">"検索"</a></li>
                            <li><a data-prefetch="true" class=if contact_active { active_cls } else { inactive_cls } href="/contact">"お問い合わせ"</a></li>
                            <li>
                                <button
                                    class="theme-toggle"
                                    type="button"
                                    aria-label="テーマ変更"
                                >
                                    <ThemeIcon />
                                </button>
                            </li>
                        </ul>
                        <div class="mobile-menu">
                            <ul>
                                <li><a data-prefetch="true" href="/">ホーム</a></li>
                                <li><a data-prefetch="true" href="/blog">ブログ</a></li>
                                <li><a data-prefetch="true" href="/profile">プロフィール</a></li>
                                <li><a data-prefetch="true" href="/search">検索</a></li>
                                <li><a data-prefetch="true" href="/contact">お問い合わせ</a></li>
                                <li>
                                    <button
                                        class="theme-toggle"
                                        type="button"
                                        aria-label="テーマ変更"
                                    >
                                        <ThemeIcon />
                                        <span>テーマ変更</span>
                                    </button>
                                </li>
                            </ul>
                        </div>
                    </div>
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
            class="show-ip"
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
    slug: Option<String>,
) -> impl IntoView {
    let pub_text = published.unwrap_or_else(|| "N/A".to_string());
    let upd_text = updated.unwrap_or_else(|| pub_text.clone());
    let pub_dt = pub_text.clone();
    let upd_dt = upd_text.clone();
    let read_label = reading_minutes.map(|m| format!("読むのに約 {} 分", m));
    view! {
        <div class="meta-row">
            <span>
                "Published: "
                <time datetime={pub_dt}>{pub_text}</time>
            </span>
            <span>
                "Updated: "
                <time datetime={upd_dt}>{upd_text}</time>
            </span>
            {read_label.map(|txt| view! { <span>{txt}</span> })}
            {slug.map(|s| {
                let typ_url = format!("/blog/{s}.typ");
                let md_url = format!("/blog/{s}.md");
                view! {
                    <span class="llm-sources">
                        "Sources for LLMs: "
                        <a href=typ_url rel="nofollow">"Typst"</a>
                        " | "
                        <a href=md_url rel="nofollow">"Markdown"</a>
                    </span>
                }
            })}
        </div>
    }
}

#[component]
fn ShowTags(tags: Vec<String>) -> impl IntoView {
    let chips = tags
        .into_iter()
        .map(|t| {
            let href = format!("/tags/{}", encode(&t));
            view! {
                <a class="blog-tag" href=href>
                    <span class="blog-tag-hash">"#"</span>
                    <span>{t}</span>
                </a>
            }
        })
        .collect::<Vec<_>>();
    view! { <div class="blog-tags not-prose">{chips}</div> }
}

#[component]
fn ShowSubtitle(text: String) -> impl IntoView {
    if text.is_empty() {
        None::<View<_>>
    } else {
        Some(view! { <p class="not-prose">{text}</p> })
    }
}

#[component]
fn ShareButtons(title: String, slug: Option<String>) -> impl IntoView {
    let slug = slug?;

    let safe_title = if title.is_empty() {
        crate::constants::SITE_NAME.to_string()
    } else {
        title.clone()
    };

    let base = SITE_URL.trim_end_matches('/');
    let page_url = format!("{base}/blog/{slug}");
    let tweet_text = format!("{}{}", safe_title, crate::constants::SITE_TITLE_POSTFIX);
    let tweet_url = format!(
        "https://twitter.com/intent/tweet?text={}&url={}",
        encode(&tweet_text),
        encode(&page_url)
    );
    let hatena_url = format!("https://b.hatena.ne.jp/entry/{}", page_url);

    Some(view! {
        <div class="share-row not-prose" aria-label="共有">
            <button
                type="button"
                class="share-btn share-native"
                data-share-native="1"
                data-share-url=page_url.clone()
                data-share-title=tweet_text.clone()
                aria-label="共有"
            >
                <svg class="share-icon" viewBox="0 0 24 24" aria-hidden="true">
                    <path
                        d="M15 5a3 3 0 1 1 .83 2.07l-6.12 3.18a3 3 0 0 1 0 3.5l6.12 3.18a3 3 0 1 1-.78 1.84l-6.12-3.18a3 3 0 1 1 0-4.34l6.12-3.18A3 3 0 0 1 15 5Z"
                        fill="currentColor"
                    />
                </svg>
                <span class="sr-only">"共有"</span>
            </button>
            <a
                class="share-btn share-twitter"
                href=tweet_url
                target="_blank"
                rel="noopener noreferrer"
                aria-label="Twitter で共有"
            >
                <svg class="share-icon" viewBox="0 0 24 24" aria-hidden="true">
                    <path
                        d="M20.77 7.19c.01.18.01.36.01.55 0 5.58-4.25 12-12.03 12-2.39 0-4.62-.69-6.5-1.88a8.52 8.52 0 0 0 6.28-1.77A4.24 4.24 0 0 1 3.9 13.7c.66.1 1.26.08 1.86-.07a4.23 4.23 0 0 1-3.4-4.16v-.05c.57.32 1.24.5 1.94.53a4.23 4.23 0 0 1-1.88-3.52c0-.78.2-1.48.57-2.1A12.03 12.03 0 0 0 12 8.27a4.23 4.23 0 0 1 7.2-3.86 8.46 8.46 0 0 0 2.68-1.03 4.25 4.25 0 0 1-1.86 2.34 8.45 8.45 0 0 0 2.43-.67 9.07 9.07 0 0 1-2.68 2.14Z"
                        fill="currentColor"
                    />
                </svg>
                <span class="sr-only">"Twitter"</span>
            </a>
            <a
                class="share-btn share-hatena"
                href=hatena_url
                target="_blank"
                rel="noopener noreferrer"
                aria-label="はてなブックマーク"
            >
                <svg class="share-icon" viewBox="0 0 24 24" aria-hidden="true">
                    <rect x="3.5" y="3.5" width="17" height="17" rx="3" fill="currentColor" />
                    <text
                        x="7.5"
                        y="15.5"
                        fill="white"
                        font-size="8"
                        font-family="Inter, 'Helvetica Neue', Arial, sans-serif"
                        font-weight="700"
                    >
                        {"B!"}
                    </text>
                </svg>
                <span class="sr-only">"はてなブックマーク"</span>
            </a>
        </div>
    })
}

#[component]
fn ThemeIcon() -> impl IntoView {
    view! {
        <span class="theme-icon-span">
            <svg
                class="theme-icon icon-sun"
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
                class="theme-icon icon-moon"
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
