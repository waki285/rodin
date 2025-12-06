use leptos::prelude::*;

use super::HeaderBar;
use super::MetaRow;
use crate::app::render::SearchHit;

#[component]
pub fn SearchPage(
    client_ip: String,
    query: String,
    results: Vec<SearchHit>,
    current_path: String,
) -> impl IntoView {
    view! {
        <div class="blog-wrapper">
            <HeaderBar
                title="すずねーう".to_string()
                subtitle=format!("{client_ip}")
                current_path=current_path.clone()
            />
            <main class="search-container">
                <div>
                    <h1>"検索"</h1>
                    <form action="/search" method="get">
                        <input
                            type="search"
                            name="q"
                            placeholder="キーワードを入力"
                            value=query.clone()
                        />
                        <button
                            type="submit"
                        >
                            "検索"
                        </button>
                    </form>
                </div>

                <div>
                    {if query.is_empty() {
                        view! { <p class="search-error">"検索キーワードを入力してください"</p> }.into_any()
                    } else if results.is_empty() {
                        view! { <p class="search-error">"該当する記事が見つかりませんでした"</p> }.into_any()
                    } else {
                        view! {
                            <ul>
                                {results
                                    .into_iter()
                                    .map(|hit| {
                                        let url = format!("/blog/{}", hit.slug);
                                        view! {
                                            <li>
                                                <a href=url.clone()>
                                                    {hit.title.clone()}
                                                </a>
                                                <div>
                                                    <MetaRow
                                                        published=hit.published_at.clone()
                                                        updated=hit.updated_at.clone()
                                                        reading_minutes=None
                                                        slug=None
                                                    />
                                                </div>
                                                <p>{hit.snippet}</p>
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        }
                        .into_any()
                    }}
                </div>
            </main>
        </div>
    }
}
