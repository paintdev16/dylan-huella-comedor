use gpui_kit::{
    App, Context, ParentElement, Styled,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Collapsible as _, Icon, Side, Sizable as _, h_flex,
        sidebar::{
            Sidebar, SidebarCollapsible, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu,
            SidebarMenuItem,
        },
        tag::Tag,
        v_flex,
    },
    prelude::FluentBuilder as _,
    px,
};

use crate::{
    layout::DesktopApp,
    state::{AppState, Page},
};

fn nav_group(
    title: &'static str,
    pages: &[Page],
    current_page: Page,
    state: &AppState,
    cx: &Context<DesktopApp>,
) -> SidebarGroup<SidebarMenu> {
    let mut menu = SidebarMenu::new();
    for &page in pages {
        if !state.auth.can_view(page) { continue; }
        let active = page == current_page;
        menu = menu.child(
            SidebarMenuItem::new(page.title())
                .icon(page.icon())
                .active(active)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.navigate_to(page, cx);
                })),
        );
    }

    SidebarGroup::new(title).child(menu)
}

/// Construye el sidebar izquierdo de navegación usando los componentes de GPUI Kit.
pub fn render_sidebar(
    state: &AppState,
    dining_hall_name: &str,
    cx: &Context<DesktopApp>,
    app_cx: &App,
) -> Sidebar<SidebarGroup<SidebarMenu>> {
    let theme = app_cx.theme();
    let collapsed = state.sidebar_collapsed;

    let brand_badge = gpui_kit::div()
        .size(if collapsed { px(30.) } else { px(34.) })
        .flex_shrink_0()
        .rounded(theme.radius)
        .bg(theme.primary)
        .text_color(theme.primary_foreground)
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(LucideIcon::Utensils).size_4());

    let header = SidebarHeader::new()
        .collapsed(collapsed)
        .when(collapsed, |this| this.p_0().justify_center())
        .when(!collapsed, |this| this.justify_start().px_2().py_1p5())
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .min_w_0()
                .when(collapsed, |this| this.justify_center().w_full())
                .child(brand_badge)
                .when(!collapsed, |this| {
                    this.child(
                        v_flex()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                gpui_kit::div()
                                    .text_sm()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.sidebar_foreground)
                                    .truncate()
                                    .child(dining_hall_name.to_string()),
                            )
                            .child(
                                gpui_kit::div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child("Gestión de Pensionistas"),
                            ),
                    )
                }),
        );

    let footer = SidebarFooter::new()
        .collapsed(collapsed)
        .child(
            Icon::new(LucideIcon::ShieldCheck)
                .size_4()
                .text_color(theme.muted_foreground),
        )
        .when(!collapsed, |this| {
            this.child(
                gpui_kit::div()
                    .flex_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Módulo 1 · Base"),
            )
            .child(Tag::secondary().small().child("v0.1.0"))
        });

    Sidebar::new("app-sidebar")
        .side(Side::Left)
        .w(px(248.))
        .collapsible(SidebarCollapsible::Icon)
        .collapsed(collapsed)
        .header(header)
        .children([
            nav_group("Principal", &Page::PRINCIPAL, state.page, state, cx),
            nav_group("Operación", &Page::OPERACION, state.page, state, cx),
            nav_group("Administración", &Page::ADMINISTRACION, state.page, state, cx),
            nav_group("Sistema", &Page::SISTEMA, state.page, state, cx),
        ])
        .footer(footer)
}


