# frozen_string_literal: true

class ApplicationController < ActionController::Base
  protect_from_forgery with: :exception, unless: -> { request.format.json? }
  allow_browser versions: :modern

  protected

  # Inertia shared props helper
  def inertia_props(**extra_props)
    shared_data = {
      flash: {
        notice: flash[:notice],
        alert: flash[:alert]
      }
    }
    shared_data.merge(extra_props)
  end
end
