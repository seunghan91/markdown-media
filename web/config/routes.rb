Rails.application.routes.draw do
  # Health check
  get "up" => "rails/health#show", as: :rails_health_check

  # API endpoints
  namespace :api do
    post "convert/url", to: "conversions#convert_url"
    post "convert/file", to: "conversions#convert_file"
  end

  # Root
  root "home#index"
end
