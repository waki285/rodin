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
        <div class="min-h-screen bg-surface text-ink">
            <HeaderBar
                title="すずねーう".to_string()
                subtitle=format!("{client_ip}")
                current_path=current_path.clone()
            />
            <main class="mx-auto max-w-3xl p-6 space-y-6">
                <div class="space-y-2">
                    <h1 class="text-3xl font-bold">"検索"</h1>
                    <form class="flex gap-2" action="/search" method="get">
                        <input
                            class="flex-1 rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm shadow-sm focus:outline-none focus:ring-2 focus:ring-slate-400 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-50 dark:focus:ring-slate-500"
                            type="search"
                            name="q"
                            placeholder="キーワードを入力"
                            value=query.clone()
                        />
                        <button
                            type="submit"
                            class="rounded-lg bg-slate-900 text-white px-4 py-2 text-sm font-semibold shadow hover:bg-slate-800 transition-colors dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-white"
                        >
                            "検索"
                        </button>
                    </form>
                </div>

                <div class="space-y-4">
                    {if query.is_empty() {
                        view! { <p class="text-sm text-slate-500 dark:text-slate-400">"検索キーワードを入力してください"</p> }.into_any()
                    } else if results.is_empty() {
                        view! { <p class="text-sm text-slate-500 dark:text-slate-400">"該当する記事が見つかりませんでした"</p> }.into_any()
                    } else {
                        view! {
                            <ul class="space-y-4">
                                {results
                                    .into_iter()
                                    .map(|hit| {
                                        let url = format!("/blog/{}", hit.slug);
                                        view! {
                                            <li class="border border-slate-200 rounded-lg p-4 bg-white shadow-sm hover:border-slate-300 transition dark:border-slate-700 dark:bg-slate-800 dark:hover:border-slate-500">
                                                <a class="text-lg font-semibold text-slate-900 hover:text-slate-700 dark:text-slate-100 dark:hover:text-white" href=url.clone()>
                                                    {hit.title.clone()}
                                                </a>
                                                <div class="text-xs text-slate-500 dark:text-slate-400 mt-1">
                                                    <MetaRow
                                                        published=hit.published_at.clone()
                                                        updated=hit.updated_at.clone()
                                                        reading_minutes=None
                                                    />
                                                </div>
                                                <p class="text-sm text-slate-700 dark:text-slate-200 mt-2 line-clamp-3">{hit.snippet}</p>
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
