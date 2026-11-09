pub mod admin;
pub mod user;

use crate::{config::Config, HandlerResult, State};
use teloxide::{
    dispatching::{dialogue::{self, InMemStorage}, UpdateHandler},
    dptree,
    prelude::*,
};

pub fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    let message_handler = Update::filter_message()
        // Если пишет админ -> отправляем в admin.rs
        .branch(
            dptree::filter(|msg: Message, cfg: Config| {
                msg.from().as_ref().map(|u| u.id.0 == cfg.admin_id).unwrap_or(false)
            })
                .endpoint(admin::handle_admin_message),
        )
        // Если пишет обычный пользователь -> отправляем в user.rs
        .branch(dptree::endpoint(user::handle_user_message));

    let callback_handler = Update::filter_callback_query()
        // Нажатия на Inline-кнопки (Одобрить / Отклонить) обрабатывает админ
        .branch(
            dptree::filter(|q: CallbackQuery, cfg: Config| q.from.id.0 == cfg.admin_id)
                .endpoint(admin::handle_admin_callback),
        );

    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(message_handler)
        .branch(callback_handler)
}