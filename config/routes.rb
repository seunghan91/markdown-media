Rails.application.routes.draw do
  # Holiday routes
  get '/holidays/:year/:month', to: 'holidays#index'
  
  namespace :api do
    # Holiday API routes
    get '/holidays/:year/:month', to: 'holidays#index'
  end
end